import Foundation
import Observation
import DenAPI
import UIKit

@MainActor @Observable final class AppStore {
    var origin: URL
    var user: API.User?
    var channels: [API.Channel] = []
    var categories: [API.Category] = []
    var users: [API.User] = []
    var readStates: [API.ChannelReadState] = []
    var messages: [String: [API.Message]] = [:]
    var messageHasNewer: [String: Bool] = [:]
    var presence: Set<String> = []
    var typing: [String: [String: Date]] = [:]
    var callStates: [API.CallState] = []
    var instanceName = "Den"
    var offline = false
    var busy = false
    var error: String?
    var selectedChannelId: String?
    var targetMessageId: String?
    var theme: ThemeStore
    var preferences = API.NotificationPreferences(dms: true, mentions: true, subscribedChannelIds: [])
    var hosts: [API.Host] = []
    var grants: [API.Grant] = []
    var pendingUploads: [PendingUpload] = []
    var calls: CallController?
    var voip: VoIPPushController?
    @ObservationIgnored var service: DenService?
    @ObservationIgnored var generation = UUID()
    @ObservationIgnored var socket: URLSessionWebSocketTask?
    @ObservationIgnored var socketLoop: Task<Void, Never>?
    @ObservationIgnored var uploadTasks: [UUID: Task<Void, Never>] = [:]
    @ObservationIgnored var lastTyping = Date.distantPast
    @ObservationIgnored var notifications: NotificationController?

    init() {
        var address = UserDefaults.standard.string(forKey: "den.activeOrigin") ?? "https://denchat.app"
        #if DEBUG && targetEnvironment(simulator)
        if let fixture = DebugFixture.credentials { address = fixture.origin }
        #endif
        let origin = (try? ServerOrigin.canonical(address)) ?? URL(string: "https://denchat.app")!
        self.origin = origin
        #if DEBUG && targetEnvironment(simulator)
        if DebugFixture.credentials != nil && ProcessInfo.processInfo.arguments.contains("--den-ui-reset") {
            try? SessionVault.delete(origin); OfflineCache.remove(origin: origin)
        }
        #endif
        theme = ThemeStore(origin: origin)
    }
    func restore() async {
        do {
            guard let token = try SessionVault.read(origin) else { return }
            // A cold PushKit answer may have already restored this same credential.
            if service == nil { service = DenService(origin: origin, token: token) }
            if let cached = OfflineCache.read(origin: origin) {
                user = cached.user; channels = cached.channels; categories = cached.categories
                users = cached.users; messages = cached.messages; readStates = cached.readStates
                instanceName = cached.instanceName; offline = true
            }
            try await refresh()
            connectSocket()
            await notifications?.registerIfAuthorized()
            await voip?.sessionRestored()
        } catch {
            if DenFailure.unauthorized(error) { clearSession(); self.error = DenFailure.signedOut.localizedDescription }
            else {
                offline = true
                // Restoring cached text offline is an expected state, not a blocking alert.
                if user == nil { report(error) }
                connectSocket()
            }
        }
    }
    func login(origin address: String, username: String, password: String) async throws {
        guard !busy else { return }
        busy = true; error = nil
        defer { busy = false }
        let nextOrigin = try ServerOrigin.canonical(address)
        let temporary = DenService(origin: nextOrigin)
        defer { temporary.close() }
        let session = try await temporary.login(username: username, password: password)
        await calls?.stopForSessionChange()
        try SessionVault.save(session.token, origin: nextOrigin)
        stopNetwork()
        if user?.id != session.user.id || origin != nextOrigin { OfflineCache.remove(origin: nextOrigin); messages = [:] }
        origin = nextOrigin; theme.reset(origin: nextOrigin); user = session.user
        service = DenService(origin: nextOrigin, token: session.token)
        UserDefaults.standard.set(nextOrigin.absoluteString, forKey: "den.activeOrigin")
        try await refresh()
        connectSocket()
        await notifications?.requestAfterLogin()
        await voip?.resumeAfterLogin()
    }
    func activeService() throws -> DenService {
        guard let service else { throw DenFailure.signedOut }
        return service
    }
    func check(_ expected: UUID) throws { guard generation == expected else { throw CancellationError() } }
    func refresh() async throws {
        let service = try activeService(), expected = generation
        async let identity = service.me()
        async let fetchedChannels = service.channels()
        async let fetchedCategories = service.categories()
        async let fetchedUsers = service.users()
        async let reads = service.readStates()
        async let appearance = service.appearance()
        async let prefs = service.preferences()
        async let online = service.presence()
        async let calls = service.calls()
        async let instance = service.instance()
        let result = try await (identity, fetchedChannels, fetchedCategories, fetchedUsers, reads, appearance, prefs, online, calls, instance)
        try check(expected)
        if let oldUser = user, oldUser.id != result.0.id { messages = [:]; OfflineCache.remove(origin: origin) }
        user = result.0; channels = result.1; categories = result.2; users = result.3
        readStates = result.4; theme.receive(result.5); preferences = result.6
        presence = Set(result.7.onlineUserIds); callStates = result.8; instanceName = result.9.instanceName
        self.calls?.session.updateNames(Dictionary(uniqueKeysWithValues: users.map { ($0.id, $0.displayName) }))
        let visible = Set(channels.map(\.id))
        messages = messages.filter { visible.contains($0.key) }
        if let selectedChannelId, !visible.contains(selectedChannelId) { self.selectedChannelId = nil }
        offline = false
        if let selectedChannelId, channels.first(where: { $0.id == selectedChannelId })?.kind != .voice {
            try await loadConversation(channelId: selectedChannelId)
        }
        saveCache(); await notifications?.updateBadge()
        // Read-only machine/access lists cannot prevent chat from booting.
        let fetchedHosts = (try? await service.hosts()) ?? []; try check(expected); hosts = fetchedHosts
        let fetchedGrants = (try? await service.grants()) ?? []; try check(expected); grants = fetchedGrants
    }
    func loadMessages(channelId: String, before: String? = nil) async throws {
        let service = try activeService(), expected = generation
        let tail = try await service.messages(channelId: channelId, before: before)
        try check(expected)
        if before == nil {
            messages[channelId] = tail.sorted { $0.id < $1.id }; messageHasNewer[channelId] = false
        }
        else { merge(tail, channelId: channelId) }
        offline = false; saveCache()
    }
    func fetchMessage(id: String, channelId: String) async throws -> API.Message {
        let expected = generation
        let message = try await activeService().message(id: id)
        try check(expected)
        guard message.channelId == channelId else { throw DenFailure.invalidResponse }
        return message
    }
    func selectChannel(_ id: String, messageId: String? = nil) {
        selectedChannelId = id; targetMessageId = messageId
    }
    func loadConversation(channelId: String) async throws {
        // The reconnect loop refetches before clearing offline. Browsing the cache must
        // not issue a request per room or turn an expected outage into modal errors.
        guard !offline else { return }
        let service = try activeService(), expected = generation
        let target = selectedChannelId == channelId ? targetMessageId : nil
        let unread = readStates.first { $0.channelId == channelId }
        try await loadMessages(channelId: channelId)
        let newest = messages[channelId]?.last?.id
        if let target, !(messages[channelId] ?? []).contains(where: { $0.id == target }) {
            let anchor = try await service.message(id: target)
            guard anchor.channelId == channelId else { throw DenFailure.invalidResponse }
            async let earlier = service.messages(channelId: channelId, before: target)
            async let later = service.messages(channelId: channelId, after: target)
            let window = try await earlier + [anchor] + later
            try check(expected); messages[channelId] = window.sorted { $0.id < $1.id }
        } else if target == nil, let unread, unread.unreadCount > 0 {
            let firstUnread = try await service.messages(channelId: channelId, after: unread.lastReadId ?? "")
            try check(expected)
            messages[channelId] = firstUnread.sorted { $0.id < $1.id }
        }
        messageHasNewer[channelId] = messages[channelId]?.last?.id != newest && newest != nil
    }
    func loadNewerMessages(channelId: String) async throws {
        let expected = generation
        let next = try await activeService().messages(channelId: channelId, after: messages[channelId]?.last?.id ?? "")
        try check(expected); merge(next, channelId: channelId)
        messageHasNewer[channelId] = next.count == 100
        saveCache()
    }
    func merge(_ values: [API.Message], channelId: String) {
        var byId = Dictionary(uniqueKeysWithValues: (messages[channelId] ?? []).map { ($0.id, $0) })
        for message in values { byId[message.id] = message }
        messages[channelId] = byId.values.sorted { $0.id < $1.id }
    }
    func send(channelId: String, content: String, replyTo: String?) async throws {
        let service = try activeService(), expected = generation
        let uploads = pendingUploads.filter { $0.channelId == channelId }
        guard uploads.allSatisfy({ $0.upload?.complete == true }) else { throw DenFailure.server(400, "Wait for attachments to finish uploading.") }
        let message = try await service.send(channelId: channelId, content: content, replyTo: replyTo, uploads: uploads.compactMap { $0.upload?.id })
        try check(expected)
        let refreshTail = messageHasNewer[channelId] == true
        if refreshTail {
            // Sending jumps to the latest message. Do not join two history windows across a gap.
            messages[channelId] = [message]; messageHasNewer[channelId] = false
        } else { merge([message], channelId: channelId) }
        // The POST committed. Never leave its attachments retryable because a later read fails.
        for upload in uploads { discardPending(id: upload.id) }
        saveCache()
        if refreshTail {
            do { try await loadMessages(channelId: channelId) }
            catch {
                // The confirmed message stays visible, and Load earlier can recover its history.
                if generation == expected, DenFailure.unauthorized(error) { report(error) }
            }
        }
    }
    func edit(message: API.Message, content: String) async throws {
        let expected = generation
        let updated = try await activeService().edit(id: message.id, content: content)
        try check(expected); merge([updated], channelId: message.channelId); saveCache()
    }
    func delete(message: API.Message) async throws {
        let expected = generation
        try await activeService().delete(id: message.id); try check(expected)
        messages[message.channelId]?.removeAll { $0.id == message.id }; saveCache()
    }
    func react(message: API.Message, emoji: String) async throws {
        let expected = generation
        let remove = message.reactions?.first { $0.emoji == emoji }?.userIds.contains(user?.id ?? "") == true
        try await activeService().react(id: message.id, emoji: emoji, remove: remove); try check(expected)
        let updated = try await fetchMessage(id: message.id, channelId: message.channelId)
        merge([updated], channelId: message.channelId)
    }
    func openDM(userIds: [String]) async throws -> API.Channel {
        let expected = generation
        let channel = try await activeService().openDM(userIds: userIds); try check(expected)
        if !channels.contains(where: { $0.id == channel.id }) { channels.append(channel) }
        selectChannel(channel.id); return channel
    }
    func markRead(channelId: String, messageId: String) async throws {
        let expected = generation
        let state = try await activeService().markRead(channelId: channelId, messageId: messageId)
        try check(expected); readStates.removeAll { $0.channelId == channelId }; readStates.append(state)
        saveCache(); await notifications?.updateBadge()
    }
    func search(query: String, channelId: String?) async throws -> [API.Message] {
        let expected = generation
        let values = try await activeService().search(query: query, channelId: channelId)
        try check(expected); return values
    }
    func saveAppearance(_ value: API.Appearance) async throws {
        let expected = generation
        let saved = try await activeService().saveAppearance(value)
        try check(expected); theme.receive(saved)
    }
    func savePreferences(_ value: API.NotificationPreferences) async throws {
        let expected = generation
        let saved = try await activeService().savePreferences(value)
        try check(expected); preferences = saved
        let reads = try await activeService().readStates(); try check(expected); readStates = reads
    }
    func logout() async throws {
        // Do not silently abandon a still-registered push endpoint on failure.
        do {
            try await cancelPendingUploads()
            await calls?.stopForSessionChange()
            try await voip?.unregisterForLogout()
            try await notifications?.unregister()
            try await activeService().logout()
            try SessionVault.delete(origin)
            clearSession()
        } catch {
            if !DenFailure.unauthorized(error) {
                await notifications?.resumeAfterInterruptedLogout()
                await voip?.resumeAfterLogin()
            }
            throw error
        }
    }
    func stopNetwork() {
        calls?.sessionInvalidated()
        generation = UUID(); socketLoop?.cancel(); socketLoop = nil
        socket?.cancel(with: .goingAway, reason: nil); socket = nil
        for task in uploadTasks.values { task.cancel() }; uploadTasks = [:]
        service?.close(); service = nil
    }
    func clearSession() {
        stopNetwork(); try? SessionVault.delete(origin); OfflineCache.remove(origin: origin)
        user = nil; channels = []; categories = []; users = []; readStates = []; messages = [:]; messageHasNewer = [:]
        callStates = []; presence = []; typing = [:]; selectedChannelId = nil; targetMessageId = nil
        for item in pendingUploads { try? FileManager.default.removeItem(at: item.localURL) }
        pendingUploads = []; hosts = []; grants = []; offline = false
    }
    func saveCache() {
        guard let user else { return }
        try? OfflineCache.save(.init(user: user, channels: channels, categories: categories, users: users,
          readStates: readStates, messages: messages.mapValues { Array($0.suffix(100)) }, instanceName: instanceName), origin: origin)
    }
    func userName(_ id: String) -> String { users.first { $0.id == id }?.displayName ?? "Someone" }
    func report(_ failure: Error) {
        // SwiftUI cancels view-owned URLSession requests on navigation. OpenAPI wraps
        // URLError.cancelled in ClientError; this is not a connectivity failure.
        guard !Task.isCancelled, !DenFailure.cancelled(failure) else { return }
        if DenFailure.unauthorized(failure) { clearSession(); error = DenFailure.signedOut.localizedDescription }
        else { error = DenFailure.present(failure) }
    }
}
