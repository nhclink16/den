import OSLog
import UIKit
import UserNotifications
import WebKit

// Notifications in Den Shell. The shell owns the iPhone's side: the permission, the device token, the app badge and
// taps. The page owns the account's side: it gives the server the token for whoever is signed in (POST /devices),
// keeps the badge at the server's count and opens what a tap points at. The page reaches this through
// `window.denShell.push(…)`, next to `call`; pages and shells without it carry on as before.

private let log = Logger(subsystem: "app.denchat.shell", category: "push")

/// One message from the page, checked before anything acts on it.
enum PushCommand: Equatable {
    /// Ask for permission if iOS hasn't yet (it shows its prompt only once), then register with Apple.
    case register
    case status
    /// Hand over the notification the person tapped, once.
    case take
    case badge(Int)
    case signedOut
    case openSettings

    init(_ body: Any) throws {
        guard let body = body as? [String: Any], let type = body["type"] as? String else { throw CallError(message: "Unreadable notification message.") }
        switch type {
        case "register": self = .register
        case "status": self = .status
        case "take": self = .take
        case "badge":
            guard let value = body["count"] as? NSNumber, CFGetTypeID(value) != CFBooleanGetTypeID(), value.intValue >= 0,
                  Double(value.intValue) == value.doubleValue else { throw CallError(message: "Missing count.") }
            self = .badge(min(value.intValue, 9999))
        case "signedOut": self = .signedOut
        case "settings": self = .openSettings
        default: throw CallError(message: "Unknown notification message.")
        }
    }
}

/// What a tapped notification points at, read from the server's payload (crates/den-server/src/push.rs).
/// Only Den ids pass: the page puts them in a path.
struct PushTap: Equatable, Encodable {
    let channelId: String
    let messageId: String?
    let threadId: String?

    init(channelId: String, messageId: String? = nil, threadId: String? = nil) {
        self.channelId = channelId
        self.messageId = messageId
        self.threadId = threadId
    }

    init?(_ info: [AnyHashable: Any]) {
        guard let channel = Self.id(info["channel_id"]) else { return nil }
        self.init(channelId: channel, messageId: Self.id(info["message_id"]), threadId: Self.id(info["thread_id"]))
    }

    static func id(_ value: Any?) -> String? {
        guard let value = value as? String, (1 ... 64).contains(value.count),
              value.allSatisfy({ $0.isASCII && ($0.isUppercase || $0.isNumber) }) else { return nil }
        return value
    }

    /// The same ids, for a notification the shell makes itself.
    var userInfo: [String: String] {
        var info = ["channel_id": channelId]
        info["message_id"] = messageId
        info["thread_id"] = threadId
        return info
    }
}

/// A reply typed into a Den notification on the lock screen or a banner. iOS may start the shell in the background
/// with no page loaded, so the shell sends it itself, with the page's own sign-in: WebKit's `den_session` cookie, read
/// when the reply is sent and never kept. The server's logout clears that cookie, so nothing outlives a sign-out.
/// The cookie goes as a bearer token, which the server takes for the same session without the page's CSRF token,
/// and only to Den's own origin, for the four requests below.
struct QuickReply: Equatable {
    static let action = "reply"
    /// The server names it in each message alert (`aps.category`, crates/den-server/src/push.rs).
    static var category: UNNotificationCategory {
        // Unlocking first means someone holding a locked phone can't post as its owner.
        let reply = UNTextInputNotificationAction(identifier: action, title: "Reply", options: [.authenticationRequired],
                                                  textInputButtonTitle: "Send", textInputPlaceholder: "Reply")
        return UNNotificationCategory(identifier: "message", actions: [reply], intentIdentifiers: [])
    }

    /// Where it goes: the thread when the notification was a thread reply, otherwise the room or DM.
    let to: PushTap
    /// Whom the notification was for. The reply is sent only from that account.
    let userId: String
    let text: String

    init(to: PushTap, userId: String, text: String) {
        self.to = to
        self.userId = userId
        self.text = text
    }

    init?(_ info: [AnyHashable: Any], text: String) {
        // A thread id that doesn't read must not turn a thread reply into a room post.
        guard info["type"] as? String == "notification", let to = PushTap(info), to.messageId != nil,
              (info["thread_id"] == nil) == (to.threadId == nil), let user = PushTap.id(info["to_user_id"]) else { return nil }
        self.init(to: to, userId: user, text: text)
    }

    enum Failure: Equatable {
        case signedOut, otherAccount, failed
        /// The reply may have reached the server before the connection went.
        case unknown
    }

    enum Outcome: Equatable {
        /// With the app icon's new number, when the server gave it.
        case sent(badge: Int?)
        case notSent(Failure)
    }

    /// The page's sign-in for Den's host, if it hasn't expired.
    static func session(in cookies: [HTTPCookie], den: Origin, now: Date = .now) -> String? {
        cookies.first {
            $0.name == "den_session" && $0.domain.lowercased() == den.host && $0.path == "/"
                && ($0.isSecure || den.scheme == "http") && ($0.expiresDate ?? .distantFuture) > now
                && !$0.value.isEmpty && $0.value.allSatisfy(\.isHexDigit)
        }?.value
    }

    static func request(_ method: String, _ path: String, body: [String: Any]? = nil, den: Origin, session: String) -> URLRequest {
        var request = URLRequest(url: URL(string: den.serialized + path)!)
        request.httpMethod = method
        request.setValue("Bearer \(session)", forHTTPHeaderField: "Authorization")
        request.setValue("application/json", forHTTPHeaderField: "Accept")
        if let body {
            request.httpBody = try? JSONSerialization.data(withJSONObject: body)
            request.setValue("application/json", forHTTPHeaderField: "Content-Type")
        }
        return request
    }

    func post(den: Origin, session: String) -> URLRequest {
        var body: [String: Any] = ["content": text]
        body["thread_id"] = to.threadId
        return Self.request("POST", "/channels/\(to.channelId)/messages", body: body, den: den, session: session)
    }

    /// Through the message the notification showed, as the page marks what's on screen: later ones stay unread.
    /// The reply itself never counts as unread.
    func read(den: Origin, session: String) -> URLRequest {
        let message = to.messageId ?? ""
        if let thread = to.threadId { return Self.request("PUT", "/threads/\(thread)/read", body: ["message_id": message], den: den, session: session) }
        return Self.request("PUT", "/channels/\(to.channelId)/read", body: ["message_id": message, "roots_only": true], den: den, session: session)
    }

    typealias Transport = @Sendable (URLRequest) async throws -> (Data, Int)

    /// Checks who the sign-in belongs to, posts, then marks the conversation read and fetches the badge. The same
    /// session makes every request, so the person checked is the person who posts.
    func send(den: Origin, session: String?, transport: Transport) async -> Outcome {
        guard let session else { return .notSent(.signedOut) }
        func json(_ data: Data) -> Any? { try? JSONSerialization.jsonObject(with: data) }
        var posting = false
        do {
            let (me, status) = try await transport(Self.request("GET", "/users/me", den: den, session: session))
            if status == 401 { return .notSent(.signedOut) }
            guard status == 200, let id = (json(me) as? [String: Any])?["id"] as? String else { return .notSent(.failed) }
            guard id == userId else { return .notSent(.otherAccount) }
            posting = true
            let (_, posted) = try await transport(post(den: den, session: session))
            if posted == 401 { return .notSent(.signedOut) }
            // The server saves the message before it answers, so a server error may still have posted it.
            if posted >= 500 { return .notSent(.unknown) }
            guard posted == 200 else { return .notSent(.failed) }
            // The reply is in. What's left only tidies the badge; the page catches up if it fails.
            _ = try? await transport(read(den: den, session: session))
            guard let counts = try? await transport(Self.request("GET", "/users/me/read-state", den: den, session: session)),
                  counts.1 == 200, let states = json(counts.0) as? [[String: Any]] else { return .sent(badge: nil) }
            return .sent(badge: states.reduce(0) { $0 + (($1["notification_count"] as? Int) ?? 0) })
        } catch {
            let unsent: [URLError.Code] = [.notConnectedToInternet, .cannotFindHost, .cannotConnectToHost, .dnsLookupFailed]
            if posting, !unsent.contains((error as? URLError)?.code ?? .unknown) { return .notSent(.unknown) }
            return .notSent(.failed)
        }
    }

    /// Ephemeral: no cookies, cache or credentials of its own, and redirects refused so the session can't follow one.
    static let http: URLSession = {
        let config = URLSessionConfiguration.ephemeral
        config.httpCookieStorage = nil
        config.httpShouldSetCookies = false
        config.urlCache = nil
        config.urlCredentialStorage = nil
        config.timeoutIntervalForRequest = 8
        return URLSession(configuration: config, delegate: NoRedirects(), delegateQueue: nil)
    }()

    static func transport(_ request: URLRequest) async throws -> (Data, Int) {
        let (data, response) = try await http.data(for: request)
        return (data, (response as? HTTPURLResponse)?.statusCode ?? 0)
    }
}

/// Tells iOS a reply is done, once, and gives back its background time.
@MainActor private final class Finish {
    private var done: (() -> Void)?
    var background = UIBackgroundTaskIdentifier.invalid

    init(_ done: @escaping () -> Void) { self.done = done }

    func callAsFunction() {
        guard let done else { return }
        self.done = nil
        done()
        if background != .invalid { UIApplication.shared.endBackgroundTask(background) }
    }
}

@MainActor private final class Once {
    private var taken = false
    func take() -> Bool {
        defer { taken = true }
        return !taken
    }
}

final class NoRedirects: NSObject, URLSessionTaskDelegate, Sendable {
    func urlSession(_ session: URLSession, task: URLSessionTask, willPerformHTTPRedirection response: HTTPURLResponse,
                    newRequest request: URLRequest) async -> URLRequest? { nil }
}

struct PushStatus: Encodable {
    var type = "status"
    let permission: String
    let token: String?
    let environment: String
    let appVersion: String

    static func permission(_ status: UNAuthorizationStatus) -> String {
        switch status {
        case .authorized, .provisional, .ephemeral: "granted"
        case .denied: "denied"
        default: "undecided"
        }
    }
}

// iOS calls the notification delegate on the main thread; `@preconcurrency` lets the main-actor methods below stand in
// for its plain requirements, and checks that thread when they run.
@MainActor final class PushBridge: NSObject, WKScriptMessageHandlerWithReply, @preconcurrency UNUserNotificationCenterDelegate {
    static let shared = PushBridge()

    private var den: Origin?
    weak var webView: WKWebView?
    /// The token Apple gave this install, kept until sign-out. Never logged.
    private var token: String?
    private var tapped: PushTap?
    /// Counts sign-outs, so a registration that was waiting on iOS when one happened doesn't switch push back on.
    private var signOuts = 0
    /// Whether a token from Apple is wanted now: not after sign-out, until the page registers again.
    private var registering = false
    /// Replies being sent from notifications, stopped at sign-out.
    private var replies: Set<Task<Void, Never>> = []

    /// Adds `push` and `onPush` to the `window.denShell` that CallBridge's script made, so install after it.
    func install(in controller: WKUserContentController, den: Origin) {
        self.den = den
        // A JSON string literal is a valid JavaScript string literal.
        let origin = String(data: try! JSONEncoder().encode(den.serialized), encoding: .utf8)!
        let source = """
        if (location.origin === \(origin) && window.denShell) Object.assign(window.denShell, {
          push: m => window.webkit.messageHandlers.denPush.postMessage(m).then(r => r == null ? null : JSON.parse(r)),
          onPush: null,
        })
        """
        controller.addUserScript(WKUserScript(source: source, injectionTime: .atDocumentStart, forMainFrameOnly: true))
        controller.addScriptMessageHandler(self, contentWorld: .page, name: "denPush")
    }

    func userContentController(_ controller: WKUserContentController, didReceive message: WKScriptMessage) async -> (Any?, String?) {
        guard let den, CallBridge.allows(isMainFrame: message.frameInfo.isMainFrame, origin: Origin(message.frameInfo.securityOrigin), den: den) else {
            log.error("notification message refused from \(message.frameInfo.securityOrigin.host, privacy: .public)")
            return (nil, "Not allowed.")
        }
        do {
            switch try PushCommand(message.body) {
            case .register: return (try Self.json(await register()), nil)
            case .status: return (try Self.json(await status()), nil)
            case .take:
                let tap = tapped
                tapped = nil
                return (try tap.map { try Self.json($0) }, nil)
            case .badge(let count):
                try? await UNUserNotificationCenter.current().setBadgeCount(count)
                return (nil, nil)
            case .signedOut:
                await signedOut()
                return (nil, nil)
            case .openSettings:
                if let url = URL(string: UIApplication.openNotificationSettingsURLString) { _ = await UIApplication.shared.open(url) }
                return (nil, nil)
            }
        } catch {
            return (nil, error.localizedDescription)
        }
    }

    private func status() async -> PushStatus {
        let authorization = await UNUserNotificationCenter.current().notificationSettings().authorizationStatus
        let version = Bundle.main.object(forInfoDictionaryKey: "CFBundleShortVersionString") as? String ?? "0"
        let build = Bundle.main.object(forInfoDictionaryKey: "CFBundleVersion") as? String ?? "0"
        return PushStatus(permission: PushStatus.permission(authorization), token: token,
                          environment: Bundle.main.object(forInfoDictionaryKey: "DenAPNSEnvironment") as? String ?? "sandbox",
                          appVersion: "\(version) (\(build))")
    }

    private func register() async -> PushStatus {
        let started = signOuts
        let center = UNUserNotificationCenter.current()
        if await center.notificationSettings().authorizationStatus == .notDetermined {
            do { _ = try await center.requestAuthorization(options: [.alert, .badge, .sound]) }
            catch { log.error("notification permission failed: \(error, privacy: .public)") }
        }
        let current = await status()
        guard current.permission == "granted", signOuts == started else { return current }
        // Apple asks for this at every launch; the token arrives in AppDelegate, then goes to the page.
        registering = true
        UIApplication.shared.registerForRemoteNotifications()
        #if DEBUG && targetEnvironment(simulator)
        // A simulator in Xcode Cloud may get no token from Apple; the UI check passes a made-up one.
        if token == nil, let fake = UserDefaults.standard.string(forKey: "DenTestPushToken") {
            token = fake
            return await status()
        }
        #endif
        return current
    }

    private func signedOut() async {
        // The server drops this sign-in's registration at logout. Unregistering also covers a logout that never
        // reached the server: Apple then refuses the old token, and the server forgets it.
        token = nil
        tapped = nil
        signOuts += 1
        registering = false
        for reply in replies { reply.cancel() }
        UIApplication.shared.unregisterForRemoteNotifications()
        let center = UNUserNotificationCenter.current()
        center.removeAllDeliveredNotifications()
        try? await center.setBadgeCount(0)
    }

    // MARK: From iOS

    func received(token: String) {
        guard registering, token != self.token else { return }
        self.token = token
        Task {
            let status = await status()
            send(try? Self.json(status))
        }
    }

    // Completion handlers, called on the main thread: UIKit aborts the app when a tap's answer comes back from
    // another thread, which the `async` forms of these did (seen in Xcode Cloud).

    /// Kept until the page takes it: after a cold launch the page may still be loading or signing in.
    func userNotificationCenter(_ center: UNUserNotificationCenter, didReceive response: UNNotificationResponse,
                                withCompletionHandler completionHandler: @escaping () -> Void) {
        if response.actionIdentifier == QuickReply.action {
            reply((response as? UNTextInputNotificationResponse)?.userText ?? "", to: response.notification.request.content.userInfo,
                  then: completionHandler)
            return
        }
        let tap = PushTap(response.notification.request.content.userInfo)
        log.info("notification tapped, \(tap == nil ? "not Den's" : "for a Den room", privacy: .public)")
        if let tap {
            tapped = tap
            send(#"{"type":"tap"}"#)
        }
        completionHandler()
    }

    func userNotificationCenter(_ center: UNUserNotificationCenter, willPresent notification: UNNotification,
                                withCompletionHandler completionHandler: @escaping (UNNotificationPresentationOptions) -> Void) {
        let info = notification.request.content.userInfo
        // A reply that didn't go always shows, even over its own conversation.
        if info["type"] as? String == "reply_failed" { return completionHandler([.banner, .list, .sound]) }
        let tap = PushTap(info)
        Task { completionHandler(await presentation(tap)) }
    }

    // MARK: Replies

    /// iOS gives an action a short while in the background. iOS hears it's done when the reply finishes, after 20
    /// seconds, or when iOS calls time, whichever comes first.
    private func reply(_ text: String, to info: [AnyHashable: Any], then done: @escaping () -> Void) {
        guard !text.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else { return done() }
        let reply = QuickReply(info, text: text), to = PushTap(info), started = signOuts
        let finish = Finish(done)
        let job = Task {
            await deliver(reply, text: text, to: to, started: started)
            finish()
        }
        replies.insert(job)
        finish.background = UIApplication.shared.beginBackgroundTask(withName: "Den reply") {
            job.cancel()
            finish()
        }
        Task {
            try? await Task.sleep(for: .seconds(20))
            job.cancel()
            finish()
        }
        Task {
            await job.value
            replies.remove(job)
        }
    }

    private func deliver(_ reply: QuickReply?, text: String, to: PushTap?, started: Int) async {
        let den = Origin(startURL)!
        var outcome = QuickReply.Outcome.notSent(.failed)
        if let reply, let cookies = await cookies() {
            outcome = await reply.send(den: den, session: QuickReply.session(in: cookies, den: den), transport: QuickReply.transport)
        }
        // Signed out meanwhile: the badge and the text belong to the account that left.
        guard signOuts == started else {
            log.info("notification reply ended after sign-out")
            return
        }
        switch outcome {
        case .sent(let badge):
            log.info("notification reply sent")
            if let badge { try? await UNUserNotificationCenter.current().setBadgeCount(min(badge, 9999)) }
        case .notSent(let why):
            log.error("notification reply not sent: \(String(describing: why), privacy: .public)")
            await notSent(text, why, to: to)
        }
    }

    /// WebKit's cookies, or nil if it doesn't answer within a few seconds.
    private func cookies() async -> [HTTPCookie]? {
        let store = WKWebsiteDataStore.default().httpCookieStore
        return await withCheckedContinuation { (answer: CheckedContinuation<[HTTPCookie]?, Never>) in
            let once = Once()
            Task { @MainActor in
                let cookies = await store.allCookies()
                if once.take() { answer.resume(returning: cookies) }
            }
            Task { @MainActor in
                try? await Task.sleep(for: .seconds(5))
                if once.take() { answer.resume(returning: nil) }
            }
        }
    }

    /// So nothing typed is lost: the text comes back in a notification, and a tap opens the conversation.
    private func notSent(_ text: String, _ why: QuickReply.Failure, to: PushTap?) async {
        let content = UNMutableNotificationContent()
        content.title = why == .unknown ? "Reply might not have been sent" : "Reply not sent"
        content.subtitle = switch why {
        case .signedOut: "Sign in to Den to send it."
        case .otherAccount: "Den is signed in to another account."
        case .failed: "Open Den to send it again."
        case .unknown: "Open Den to check."
        }
        content.body = text
        content.sound = .default
        content.userInfo = (to?.userInfo ?? [:]).merging(["type": "reply_failed"]) { $1 }
        let request = UNNotificationRequest(identifier: UUID().uuidString, content: content, trigger: nil)
        do { try await UNUserNotificationCenter.current().add(request) }
        catch { log.error("reply-not-sent notification failed: \(error, privacy: .public)") }
    }

    /// With Den open, the page already plays its own alert sound and keeps the badge, so iOS only shows the banner,
    /// and nothing when the page says that conversation is on screen. A page that doesn't answer gets the full alert.
    private func presentation(_ tap: PushTap?) async -> UNNotificationPresentationOptions {
        guard let tap, let webView, let json = try? Self.json(tap) else { return [.banner, .list, .sound] }
        let looking = try? await webView.callAsyncJavaScript(
            "return window.denShell?.onPush?.({ type: 'present', ...JSON.parse(s) }) ?? null",
            arguments: ["s": json], contentWorld: .page)
        switch looking as? Bool {
        case true?: return []
        case false?: return [.banner, .list]
        case nil: return [.banner, .list, .sound]
        }
    }

    private func send(_ json: String?) {
        guard let webView, let json else { return }
        webView.callAsyncJavaScript("window.denShell?.onPush?.(JSON.parse(s))", arguments: ["s": json], in: nil, in: .page) { result in
            if case .failure(let error) = result { log.error("notification event not delivered: \(error, privacy: .public)") }
        }
    }

    nonisolated static func json(_ value: some Encodable) throws -> String {
        String(data: try JSONEncoder().encode(value), encoding: .utf8)!
    }
}

/// Only for what SwiftUI can't do: the push token, and the notification delegate before launch ends.
final class AppDelegate: NSObject, UIApplicationDelegate {
    func application(_ application: UIApplication, didFinishLaunchingWithOptions launchOptions: [UIApplication.LaunchOptionsKey: Any]? = nil) -> Bool {
        // Set before launch ends, or the tap that launched the app is lost.
        UNUserNotificationCenter.current().delegate = PushBridge.shared
        UNUserNotificationCenter.current().setNotificationCategories([QuickReply.category])
        return true
    }

    func application(_ application: UIApplication, didRegisterForRemoteNotificationsWithDeviceToken deviceToken: Data) {
        PushBridge.shared.received(token: deviceToken.map { String(format: "%02x", $0) }.joined())
    }

    func application(_ application: UIApplication, didFailToRegisterForRemoteNotificationsWithError error: Error) {
        // Chat works without it; the page's Settings says this iPhone isn't connected yet.
        log.error("push registration failed: \(error, privacy: .public)")
    }
}
