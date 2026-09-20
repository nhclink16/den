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
    var typing: [Conversation: [String: Date]] = [:]
    var callStates: [API.CallState] = []
    var instanceName = "Den"
    var syncProblem: SyncProblem?
    // Both states make live actions unavailable; only a network failure is shown as Offline.
    var offline: Bool { syncProblem != nil }
    var busy = false
    var error: String?
    var selectedChannelId: String?
    var targetMessageId: String?
    var selectedThread: ThreadSelection?
    var theme: ThemeStore
    var preferences = API.NotificationPreferences(dms: true, mentions: true, subscribedChannelIds: [])
    var hosts: [API.Host] = []
    var grants: [API.Grant] = []
    var pendingUploads: [PendingUpload] = []
    var drafts: [Conversation: Draft] = [:]
    var threadMetadata: [String: API.ThreadSummary] = [:]
    var threadReadStates: [String: API.ThreadReadState] = [:]
    var threadMessages: [String: [API.Message]] = [:]
    var threadRoots: [String: API.Message] = [:]
    var threadHasNewer: [String: Bool] = [:]
    var calls: CallController?
    var voip: VoIPPushController?
    var dictation: DictationController?
    var dictationChannelId: String?
    @ObservationIgnored var service: DenService?
    @ObservationIgnored var generation = UUID()
    @ObservationIgnored let readOrder = ReadOrdering()
    @ObservationIgnored let metadataOrder = ReadOrdering()
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
                users = cached.users
                messages = cached.messages.mapValues { $0.filter { $0.threadId == nil } }
                threadMetadata = cached.threadMetadata ?? [:]
                threadReadStates = cached.threadReadStates ?? [:]
                threadMessages = cached.threadMessages ?? [:]
                threadRoots = cached.threadRoots ?? [:]
                replaceReadStates(cached.readStates)
                instanceName = cached.instanceName; syncProblem = .networkOffline
            }
            try await refresh()
            connectSocket()
            await notifications?.registerIfAuthorized()
            await voip?.sessionRestored()
        } catch {
            if DenFailure.unauthorized(error) { clearSession(); self.error = DenFailure.signedOut.localizedDescription }
            else {
                recordSyncFailure(error)
                // Restoring cached text offline is an expected state, not a blocking alert.
                if user == nil { report(error) }
                if syncProblem != .incompatibleResponse { connectSocket() }
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
        if user?.id != session.user.id || origin != nextOrigin {
            OfflineCache.remove(origin: nextOrigin); messages = [:]; resetThreads()
        }
        origin = nextOrigin; theme.reset(origin: nextOrigin); user = session.user; syncProblem = nil
        service = DenService(origin: nextOrigin, token: session.token)
        UserDefaults.standard.set(nextOrigin.absoluteString, forKey: "den.activeOrigin")
        do { try await refresh() }
        catch {
            if !DenFailure.unauthorized(error), !DenFailure.cancelled(error), syncProblem != .incompatibleResponse { connectSocket() }
            throw error
        }
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
        let expected = generation
        do { try await refreshContents() }
        catch {
            if generation == expected, !DenFailure.cancelled(error) { recordSyncFailure(error) }
            throw error
        }
    }
    private func refreshContents() async throws {
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
        if let oldUser = user, oldUser.id != result.0.id {
            messages = [:]; resetThreads(); OfflineCache.remove(origin: origin)
        }
        user = result.0; channels = result.1; categories = result.2; users = result.3
        replaceReadStates(result.4); theme.receive(result.5); preferences = result.6
        presence = Set(result.7.onlineUserIds); callStates = result.8; instanceName = result.9.instanceName
        refreshCallNames()
        let visible = Set(channels.map(\.id))
        pruneConversationCaches(visibleChannelIds: visible)
        if let selectedChannelId, !visible.contains(selectedChannelId) {
            self.selectedChannelId = nil
            selectedThread = nil
        }
        if let selectedChannelId, channels.first(where: { $0.id == selectedChannelId })?.kind != .voice {
            try await loadConversation(channelId: selectedChannelId, duringRefresh: true)
            _ = try await loadThreads(channelId: selectedChannelId, resolved: false)
            if let id = selectedThread?.threadId {
                try await hydrateThread(id: id, target: selectedThread?.targetMessageId,
                                        duringRefresh: true)
            }
        }
        syncProblem = nil
        if error == DenFailure.updateRequired.localizedDescription { error = nil }
        saveCache(); await notifications?.updateBadge()
        // Read-only machine/access lists cannot prevent chat from booting.
        let fetchedHosts = (try? await service.hosts()) ?? []; try check(expected); hosts = fetchedHosts
        let fetchedGrants = (try? await service.grants()) ?? []; try check(expected); grants = fetchedGrants
    }
    func loadMessages(channelId: String, before: String? = nil) async throws {
        let service = try activeService(), expected = generation
        let tail = try await service.messages(channelId: channelId, before: before, rootsOnly: true)
        try check(expected)
        if before == nil {
            messages[channelId] = tail.filter { $0.threadId == nil }.sorted { $0.id < $1.id }
            messageHasNewer[channelId] = false
            for message in tail { mergeInlineThread(message) }
        }
        else { merge(tail, channelId: channelId) }
        if syncProblem == .networkOffline { syncProblem = nil }
        saveCache()
    }
    func fetchMessage(id: String, channelId: String) async throws -> API.Message {
        let expected = generation
        let message = try await activeService().message(id: id)
        try check(expected)
        guard message.channelId == channelId else { throw DenFailure.invalidResponse }
        return message
    }
    func selectChannel(_ id: String, messageId: String? = nil) {
        selectedChannelId = id; targetMessageId = messageId; selectedThread = nil
    }
    func selectThread(channelId: String, rootId: String? = nil, threadId: String? = nil,
                      messageId: String? = nil) {
        selectedChannelId = channelId; targetMessageId = nil
        selectedThread = .init(channelId: channelId, rootId: rootId, threadId: threadId,
                               targetMessageId: messageId)
    }
    func loadConversation(channelId: String, duringRefresh: Bool = false) async throws {
        // The reconnect loop refetches before clearing offline. Browsing the cache must
        // not issue a request per room or turn an expected outage into modal errors.
        guard duringRefresh || !offline else { return }
        let service = try activeService(), expected = generation
        let target = selectedChannelId == channelId ? targetMessageId : nil
        let unread = readStates.first { $0.channelId == channelId }
        try await loadMessages(channelId: channelId)
        let newest = messages[channelId]?.last?.id
        if let target, !(messages[channelId] ?? []).contains(where: { $0.id == target }) {
            let anchor = try await service.message(id: target)
            guard anchor.channelId == channelId else { throw DenFailure.invalidResponse }
            async let earlier = service.messages(channelId: channelId, before: target, rootsOnly: true)
            async let later = service.messages(channelId: channelId, after: target, rootsOnly: true)
            let window = try await earlier + [anchor] + later
            try check(expected); messages[channelId] = window.filter { $0.threadId == nil }.sorted { $0.id < $1.id }
        } else if target == nil, let unread, unread.unreadCount > 0 {
            let firstUnread = try await service.messages(channelId: channelId, after: unread.lastReadId ?? "", rootsOnly: true)
            try check(expected)
            // The channel total can come entirely from collapsed conversations.
            // In that case there is no unread room window, so keep the loaded tail.
            if !firstUnread.isEmpty { messages[channelId] = firstUnread.sorted { $0.id < $1.id } }
        }
        messageHasNewer[channelId] = messages[channelId]?.last?.id != newest && newest != nil
    }
    func loadNewerMessages(channelId: String) async throws {
        let expected = generation
        let next = try await activeService().messages(channelId: channelId,
            after: messages[channelId]?.last?.id ?? "", rootsOnly: true)
        try check(expected); merge(next, channelId: channelId)
        messageHasNewer[channelId] = next.count == 100
        saveCache()
    }
    func merge(_ values: [API.Message], channelId: String) {
        var byId = Dictionary(uniqueKeysWithValues: (messages[channelId] ?? []).map { ($0.id, $0) })
        for message in values where message.threadId == nil {
            byId[message.id] = message
            mergeInlineThread(message)
        }
        messages[channelId] = byId.values.sorted { $0.id < $1.id }
    }
    @discardableResult
    func send(conversation: Conversation, content: String, replyTo: String?) async throws -> API.Message {
        let service = try activeService(), expected = generation
        let channelId = conversation.channelId
        let uploads = pendingUploads.filter { $0.conversation == conversation }
        guard uploads.allSatisfy({ $0.upload?.complete == true }) else { throw DenFailure.server(400, "Wait for attachments to finish uploading.") }
        let threadId: String?
        let target: String?
        switch conversation {
        case .room:
            threadId = nil; target = replyTo
        case let .thread(_, rootId):
            threadId = threadForRoot(rootId)?.id
            target = replyTo ?? (threadId == nil ? rootId : nil)
        }
        let message = try await service.send(channelId: channelId, content: content,
            replyTo: target, threadId: threadId, uploads: uploads.compactMap { $0.upload?.id })
        try check(expected)
        guard message.channelId == channelId else { throw DenFailure.invalidResponse }
        let refreshTail: Bool
        var confirmedThread: (id: String, rootId: String)?
        switch conversation {
        case .room:
            guard message.threadId == nil else { throw DenFailure.invalidResponse }
            refreshTail = messageHasNewer[channelId] == true
            if refreshTail {
                messages[channelId] = [message]; messageHasNewer[channelId] = false
            } else { merge([message], channelId: channelId) }
        case let .thread(_, rootId):
            guard let returnedThread = message.threadId,
                  threadId == nil || threadId == returnedThread else { throw DenFailure.invalidResponse }
            refreshTail = threadHasNewer[returnedThread] == true
            if refreshTail {
                threadMessages[returnedThread] = [message]; threadHasNewer[returnedThread] = false
            } else { mergeThreadMessages([message], threadId: returnedThread) }
            confirmedThread = (returnedThread, rootId)
            if selectedThread?.channelId == channelId,
               selectedThread?.rootId == nil || selectedThread?.rootId == rootId {
                selectedThread?.rootId = rootId
                selectedThread?.threadId = returnedThread
            }
        }
        // The POST committed. Never leave its attachments retryable because a later read fails.
        for upload in uploads { discardPending(id: upload.id) }
        saveCache()
        if let confirmedThread {
            do {
                let view = try await loadThread(id: confirmedThread.id)
                guard view.thread.rootMessageId == confirmedThread.rootId else { throw DenFailure.invalidResponse }
            } catch {
                // The message is committed and stays visible. Reconnect or a manual
                // refresh can recover metadata without making the composer retry it.
                if generation == expected, DenFailure.unauthorized(error) { report(error) }
            }
        }
        if refreshTail, case .room = conversation {
            do { try await loadMessages(channelId: channelId) }
            catch {
                // The confirmed message stays visible, and Load earlier can recover its history.
                if generation == expected, DenFailure.unauthorized(error) { report(error) }
            }
        }
        return message
    }
    func send(channelId: String, content: String, replyTo: String?) async throws {
        _ = try await send(conversation: .room(channelId), content: content, replyTo: replyTo)
    }
    func edit(message: API.Message, content: String) async throws {
        let expected = generation
        let updated = try await activeService().edit(id: message.id, content: content)
        try check(expected); mergeMessage(updated); saveCache()
    }
    func delete(message: API.Message) async throws {
        let expected = generation
        try await activeService().delete(id: message.id); try check(expected)
        if let threadId = message.threadId { threadMessages[threadId]?.removeAll { $0.id == message.id } }
        else { messages[message.channelId]?.removeAll { $0.id == message.id } }
        saveCache()
    }
    func react(message: API.Message, emoji: String) async throws {
        let expected = generation
        let current: API.Message
        if let threadId = message.threadId {
            current = threadMessages[threadId]?.first { $0.id == message.id } ?? message
        } else {
            current = messages[message.channelId]?.first { $0.id == message.id } ?? message
        }
        let remove = current.reactions?.first { $0.emoji == emoji }?.userIds.contains(user?.id ?? "") == true
        try await activeService().react(id: message.id, emoji: emoji, remove: remove); try check(expected)
        let updated = try await fetchMessage(id: message.id, channelId: message.channelId)
        mergeMessage(updated)
    }
    func openDM(userIds: [String]) async throws -> API.Channel {
        let expected = generation
        let channel = try await activeService().openDM(userIds: userIds); try check(expected)
        if !channels.contains(where: { $0.id == channel.id }) { channels.append(channel) }
        selectChannel(channel.id); return channel
    }
    func draft(_ conversation: Conversation) -> Draft { drafts[conversation] ?? Draft() }
    /// Every edit bumps the revision. Setting the same text is still an edit:
    /// A -> B -> A must not look like no change at all.
    func setDraftText(_ text: String, for conversation: Conversation) {
        var draft = self.draft(conversation)
        draft.text = text
        draft.revision += 1
        drafts[conversation] = draft
    }
    func setDraftReply(_ replyToId: String?, for conversation: Conversation) {
        var draft = self.draft(conversation)
        draft.replyToId = replyToId
        drafts[conversation] = draft
    }
    /// Captured at submit, before anything is awaited.
    func submittedIdentity(_ conversation: Conversation) -> DraftIdentity {
        let draft = self.draft(conversation)
        return DraftIdentity(conversation: conversation, replyToId: draft.replyToId, revision: draft.revision)
    }
    /// Clears only if the conversation, the quote target and the revision are all
    /// still the ones that were sent. Otherwise the draft on screen is newer and
    /// belongs to whoever typed it.
    @discardableResult
    func clearDraft(matching identity: DraftIdentity) -> Bool {
        guard submittedIdentity(identity.conversation) == identity else { return false }
        drafts[identity.conversation] = nil
        return true
    }
    func markRead(channelId: String, messageId: String) async throws {
        let expected = generation
        // Taken BEFORE the request goes out: what this response is allowed to
        // overwrite is decided by what was true when it was sent.
        let token = readOrder.begin(.channel(channelId))
        let state = try await activeService().markRead(channelId: channelId, messageId: messageId,
                                                       rootsOnly: true)
        try check(expected)
        applyChannelRead(state, token: token)
        saveCache(); await notifications?.updateBadge()
    }
    /// The explicit room-wide action keeps the legacy flat meaning. It captures
    /// the server's current flat tail before advancing room and conversation reads.
    func markAllRead(channelId: String) async throws {
        let expected = generation
        guard let marker = try await activeService().messages(channelId: channelId).last else { return }
        try check(expected)
        let token = readOrder.begin(.channel(channelId))
        let state = try await activeService().markRead(channelId: channelId, messageId: marker.id)
        try check(expected)
        applyChannelRead(state, token: token)
        try await refreshThreadStates(channelId: channelId)
        saveCache(); await notifications?.updateBadge()
    }
    /// The single place a channel read state is stored. `token` identifies a
    /// response to one of our own reads; a pushed update passes nil and wins.
    func applyChannelRead(_ state: API.ChannelReadState, token: Int? = nil) {
        guard readOrder.accept(.channel(state.channelId), token: token) else { return }
        readStates.removeAll { $0.channelId == state.channelId }
        readStates.append(state)
    }
    /// The channel snapshot supersedes channel responses already in flight.
    /// Thread reads come from their own endpoint and retain independent ordering.
    func replaceReadStates(_ states: [API.ChannelReadState]) {
        readOrder.invalidateChannels()
        readStates = states
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
        let reads = try await activeService().readStates(); try check(expected); replaceReadStates(reads)
    }
    func logout() async throws {
        dictation?.invalidateContext()
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
        dictation?.invalidateContext()
        calls?.sessionInvalidated()
        generation = UUID(); socketLoop?.cancel(); socketLoop = nil
        socket?.cancel(with: .goingAway, reason: nil); socket = nil
        for task in uploadTasks.values { task.cancel() }; uploadTasks = [:]
        service?.close(); service = nil
    }
    func clearSession() {
        stopNetwork(); try? SessionVault.delete(origin); OfflineCache.remove(origin: origin)
        // Synchronous with the account: a response from the previous account must
        // not be accepted against the next one.
        readOrder.reset()
        metadataOrder.reset()
        user = nil; channels = []; categories = []; users = []; readStates = []; messages = [:]; messageHasNewer = [:]
        callStates = []; presence = []; typing = [:]; selectedChannelId = nil; targetMessageId = nil
        selectedThread = nil; resetThreads()
        for item in pendingUploads { try? FileManager.default.removeItem(at: item.localURL) }
        pendingUploads = []; hosts = []; grants = []; syncProblem = nil
        // One account's half-written message must not appear under the next.
        drafts = [:]
        // Never let one account's names survive into the next account's call labels.
        refreshCallNames()
    }
    func saveCache() {
        guard let user else { return }
        try? OfflineCache.save(.init(user: user, channels: channels, categories: categories, users: users,
          readStates: readStates, messages: messages.mapValues { Array($0.suffix(100)) },
          instanceName: instanceName, threadMetadata: threadMetadata,
          threadReadStates: threadReadStates,
          threadMessages: threadMessages.mapValues { Array($0.suffix(100)) },
          threadRoots: threadRoots), origin: origin)
    }
    func userName(_ id: String) -> String { users.first { $0.id == id }?.displayName ?? "Someone" }

    func pruneConversationCaches(visibleChannelIds: Set<String>) {
        messages = messages.filter { visibleChannelIds.contains($0.key) }
        threadMetadata = threadMetadata.filter { visibleChannelIds.contains($0.value.channelId) }
        threadReadStates = threadReadStates.filter { visibleChannelIds.contains($0.value.channelId) }
        threadRoots = threadRoots.filter { visibleChannelIds.contains($0.value.channelId) }
        threadMessages = threadMessages.filter { id, values in
            if let channelId = values.first?.channelId { return visibleChannelIds.contains(channelId) }
            return threadMetadata[id] != nil || threadReadStates[id] != nil
        }
        let retained = Set(threadMetadata.keys).union(threadReadStates.keys).union(threadMessages.keys)
        threadHasNewer = threadHasNewer.filter { retained.contains($0.key) }
        typing = typing.filter { visibleChannelIds.contains($0.key.channelId) }
    }
    /// Every name label reads `users` or `user`, so one upsert carries a profile change into
    /// message authors, DM titles, People, Settings, mentions and initials already on screen.
    /// The event carries the whole user, so this never refetches: a rename must not discard
    /// loaded history, an unsent draft, unread counts, or the offline state being displayed.
    func apply(_ updated: API.User) {
        if let index = users.firstIndex(where: { $0.id == updated.id }) { users[index] = updated }
        else { users.append(updated) }
        if user?.id == updated.id { user = updated }
        refreshCallNames(); saveCache()
    }
    /// A live call captured its title when it was reported. Recompute it from current names
    /// so a rename does not stay stale in the dock or in CallKit for the length of the call.
    func refreshCallNames() {
        calls?.updateNames(Dictionary(uniqueKeysWithValues: users.map { ($0.id, $0.displayName) })) { [weak self] channelId, incomingFrom in
            self?.callTitle(channelId: channelId, incomingFrom: incomingFrom)
        }
    }
    /// The caller's own name labels a reported incoming call; every other call is labelled by
    /// its channel. An unknown caller or an unloaded channel keeps the title already reported.
    func callTitle(channelId: String, incomingFrom: String?) -> String? {
        if let incomingFrom { return users.first { $0.id == incomingFrom }?.displayName }
        return channels.first { $0.id == channelId }.map(channelTitle)
    }
    func report(_ failure: Error) {
        // SwiftUI cancels view-owned URLSession requests on navigation. OpenAPI wraps
        // URLError.cancelled in ClientError; this is not a connectivity failure.
        guard !Task.isCancelled, !DenFailure.cancelled(failure) else { return }
        if DenFailure.unauthorized(failure) { clearSession(); error = DenFailure.signedOut.localizedDescription }
        else {
            if DenFailure.incompatible(failure) { recordSyncFailure(failure) }
            error = DenFailure.present(failure)
        }
    }
    func recordSyncFailure(_ failure: Error) {
        guard !DenFailure.cancelled(failure) else { return }
        if DenFailure.incompatible(failure) {
            syncProblem = .incompatibleResponse
            socketLoop?.cancel(); socketLoop = nil
            socket?.cancel(with: .goingAway, reason: nil); socket = nil
        } else if syncProblem != .incompatibleResponse {
            syncProblem = .networkOffline
        }
        presence = []; typing = [:]
    }
    func retrySync() async {
        do {
            try await refresh()
            // A schema failure tore the loop down. An ordinary pull must not bounce a live socket.
            if socketLoop == nil { connectSocket() }
        } catch { report(error) }
    }
}
