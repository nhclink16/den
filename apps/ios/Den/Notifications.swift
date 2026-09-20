import Foundation
import UIKit
import UserNotifications
import DenAPI

@MainActor final class DenAppDelegate: NSObject, UIApplicationDelegate {
    let store = AppStore()
    var notifications: NotificationController?
    override init() {
        super.init()
        let notifications = NotificationController(store: store)
        self.notifications = notifications; store.notifications = notifications
        let api = NativeCallAPI(store: store)
        let calls = CallController(session: CallSession(), api: api)
        store.calls = calls
        store.voip = VoIPPushController(calls: calls, registration: api)
        let callActive: @MainActor () -> Bool = { [weak calls] in calls?.preventsDictation == true }
#if DEBUG && targetEnvironment(simulator)
        let dictation = DebugFixture.dictation(isCallActive: callActive) ?? DictationController(isCallActive: callActive)
#else
        let dictation = DictationController(isCallActive: callActive)
#endif
        store.dictation = dictation
        calls.beforeAudioPreparation = { [weak dictation] in dictation?.invalidateContext() }
    }
    func application(_ application: UIApplication, didFinishLaunchingWithOptions launchOptions: [UIApplication.LaunchOptionsKey: Any]? = nil) -> Bool {
        // PushKit may launch us with no SwiftUI scene. Reporting cannot wait for chat restore.
        store.voip?.start()
        return true
    }
    func application(_ application: UIApplication, didRegisterForRemoteNotificationsWithDeviceToken deviceToken: Data) {
        let token = deviceToken.map { String(format: "%02x", $0) }.joined()
        Task { await notifications?.registered(token: token) }
    }
    func application(_ application: UIApplication, didFailToRegisterForRemoteNotificationsWithError error: Error) {
        // Simulator or missing capability must not break text chat.
        notifications?.status = "Push registration is unavailable on this device."
    }
}

@MainActor @Observable final class NotificationController: NSObject, UNUserNotificationCenterDelegate {
    weak var store: AppStore?
    var status: String?
    private var token: String?
    private var acceptingRegistration = true
    private struct Registration: Equatable {
        let generation: UUID
        let token: String
    }
    private var registeredIdentity: Registration?
    private var attemptedIdentity: Registration?
    private var retryAfter = Date.distantPast
    @ObservationIgnored private var registrationTask: Task<Void, Never>?
    @ObservationIgnored private let authorizationStatus: @MainActor () async -> UNAuthorizationStatus
    @ObservationIgnored private let requestAuthorization: @MainActor () async throws -> Bool
    @ObservationIgnored private let registerRemote: @MainActor () -> Void
    private var pendingTap: (channel: String, message: String?, thread: String?)?
    init(store: AppStore,
         authorizationStatus: @escaping @MainActor () async -> UNAuthorizationStatus = {
             await UNUserNotificationCenter.current().notificationSettings().authorizationStatus
         },
         requestAuthorization: @escaping @MainActor () async throws -> Bool = {
             try await UNUserNotificationCenter.current().requestAuthorization(options: [.alert, .badge, .sound])
         },
         registerRemote: @escaping @MainActor () -> Void = {
             UIApplication.shared.registerForRemoteNotifications()
         }) {
        self.store = store
        self.authorizationStatus = authorizationStatus
        self.requestAuthorization = requestAuthorization
        self.registerRemote = registerRemote
        super.init()
        UNUserNotificationCenter.current().delegate = self
    }
    private func key(_ store: AppStore) -> String {
        "den.device:\(store.origin.absoluteString):\(store.user?.id ?? "")"
    }
    func requestAfterLogin() async {
        acceptingRegistration = true
        consumePendingTap()
        guard let store, store.user != nil else { return }
        let expected = store.generation
        do {
            let allowed = try await requestAuthorization()
            try store.check(expected)
            guard acceptingRegistration else { return }
            if allowed {
                registerRemote()
                if let token { await registered(token: token, retryImmediately: true) }
            }
        } catch {
            if store.generation == expected, !DenFailure.cancelled(error) { status = "Notification permission could not be checked." }
        }
    }
    func registerIfAuthorized(retryImmediately: Bool = false) async {
        consumePendingTap()
        guard acceptingRegistration, let store, store.user != nil, store.service != nil else { return }
        let expected = store.generation
        let authorization = await authorizationStatus()
        guard acceptingRegistration, store.generation == expected else { return }
        if authorization == .authorized || authorization == .provisional {
            registerRemote()
            // Retrying the cached token does not depend on another APNs callback arriving.
            if let token { await registered(token: token, retryImmediately: retryImmediately) }
        }
    }
    func registered(token: String, retryImmediately: Bool = false) async {
        self.token = token
        // Waiters recheck after every task, including a task started by another waiter.
        while let task = registrationTask { await task.value }
        guard acceptingRegistration, let store, store.user != nil, let service = store.service else { return }
        let expected = store.generation, storageKey = key(store)
        let identity = Registration(generation: expected, token: token)
        guard registeredIdentity != identity else { return }
        guard retryImmediately || attemptedIdentity != identity || Date() >= retryAfter else { return }
        attemptedIdentity = identity
        let task = Task { [weak self] in
            guard let self else { return }
            defer { registrationTask = nil }
            do {
                let version = Bundle.main.object(forInfoDictionaryKey: "CFBundleShortVersionString") as? String ?? "0"
                guard let environment = Bundle.main.object(forInfoDictionaryKey: "DenAPNSEnvironment") as? String,
                      API.DeviceEnvironment(rawValue: environment) != nil else { throw DenFailure.invalidResponse }
                let device = try await service.client.postDevices(body: .json(.init(
                    appVersion: version, clientId: VoIPPushInstallation.clientID().uuidString,
                    environment: environment, platform: .ios, purpose: .alert, token: token))).ok.body.json
                try store.check(expected)
                UserDefaults.standard.set(device.id, forKey: storageKey)
                registeredIdentity = identity; retryAfter = .distantPast; status = nil
            } catch {
                guard store.generation == expected else { return }
                retryAfter = Date().addingTimeInterval(15)
                if !DenFailure.cancelled(error) { status = "Notifications are not registered yet. Chat is still available." }
            }
        }
        registrationTask = task
        await task.value
    }
    func handleTap(channel: String, message: String?, thread: String? = nil) {
        if let store, store.user != nil {
            if let thread { store.selectThread(channelId: channel, threadId: thread, messageId: message) }
            else { store.selectChannel(channel, messageId: message) }
        } else { pendingTap = (channel, message, thread) }
    }
    private func consumePendingTap() {
        guard let pendingTap, let store, store.user != nil else { return }
        self.pendingTap = nil
        if let thread = pendingTap.thread {
            store.selectThread(channelId: pendingTap.channel, threadId: thread,
                               messageId: pendingTap.message)
        } else {
            store.selectChannel(pendingTap.channel, messageId: pendingTap.message)
        }
    }
    func unregister() async throws {
        guard let store else { return }
        acceptingRegistration = false
        await registrationTask?.value
        registrationTask = nil
        let storageKey = key(store)
        if let id = UserDefaults.standard.string(forKey: storageKey) {
            do { _ = try await store.activeService().client.deleteDevicesId(path: .init(id: id)).noContent }
            catch {
                let underlying = (error as? ClientError)?.underlyingError ?? error
                if case DenFailure.server(404, _) = underlying { }
                else if !DenFailure.unauthorized(error) { acceptingRegistration = true; throw error }
            }
            UserDefaults.standard.removeObject(forKey: storageKey)
        }
        UIApplication.shared.unregisterForRemoteNotifications()
        UNUserNotificationCenter.current().removeAllDeliveredNotifications()
        try? await UNUserNotificationCenter.current().setBadgeCount(0)
        token = nil; pendingTap = nil; registeredIdentity = nil; attemptedIdentity = nil; retryAfter = .distantPast
    }
    func updateBadge() async {
        guard let store else { return }
        let count = store.readStates.reduce(Int64(0)) { $0 + $1.notificationCount }
        try? await UNUserNotificationCenter.current().setBadgeCount(Int(min(count, 9999)))
    }
    func resumeAfterInterruptedLogout() async {
        acceptingRegistration = true
        await registerIfAuthorized(retryImmediately: true)
    }
    nonisolated func userNotificationCenter(_ center: UNUserNotificationCenter,
        didReceive response: UNNotificationResponse) async {
        let info = response.notification.request.content.userInfo
        guard let channel = info["channel_id"] as? String else { return }
        let message = info["message_id"] as? String
        let thread = info["thread_id"] as? String
        await MainActor.run { handleTap(channel: channel, message: message, thread: thread) }
    }
    nonisolated func userNotificationCenter(_ center: UNUserNotificationCenter,
        willPresent notification: UNNotification) async -> UNNotificationPresentationOptions {
        let info = notification.request.content.userInfo
        let channel = info["channel_id"] as? String
        let thread = info["thread_id"] as? String
        return await MainActor.run {
            presentationOptions(channel: channel, thread: thread)
        }
    }
    func presentationOptions(channel: String?, thread: String?) -> UNNotificationPresentationOptions {
        let showingExactConversation: Bool
        if let thread {
            showingExactConversation = store?.selectedThread?.threadId == thread
        } else {
            showingExactConversation = store?.selectedChannelId == channel && store?.selectedThread == nil
        }
        return showingExactConversation ? [.badge] : [.banner, .sound, .badge]
    }
}
