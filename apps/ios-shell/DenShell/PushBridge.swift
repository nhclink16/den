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
        func id(_ key: String) -> String? {
            guard let value = info[key] as? String, (1 ... 64).contains(value.count),
                  value.allSatisfy({ $0.isASCII && ($0.isUppercase || $0.isNumber) }) else { return nil }
            return value
        }
        guard let channel = id("channel_id") else { return nil }
        self.init(channelId: channel, messageId: id("message_id"), threadId: id("thread_id"))
    }
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

@MainActor final class PushBridge: NSObject, WKScriptMessageHandlerWithReply, UNUserNotificationCenterDelegate {
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

    // Both on the main actor: iOS finishes these on the thread they return on, and UIKit aborts the app when a tap's
    // answer comes back off the main thread (seen in Xcode Cloud).

    /// Kept until the page takes it: after a cold launch the page may still be loading or signing in.
    func userNotificationCenter(_ center: UNUserNotificationCenter, didReceive response: UNNotificationResponse) async {
        let tap = PushTap(response.notification.request.content.userInfo)
        log.info("notification tapped, \(tap == nil ? "not Den's" : "for a Den room", privacy: .public)")
        guard let tap else { return }
        tapped = tap
        send(#"{"type":"tap"}"#)
    }

    func userNotificationCenter(_ center: UNUserNotificationCenter, willPresent notification: UNNotification) async -> UNNotificationPresentationOptions {
        await presentation(PushTap(notification.request.content.userInfo))
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
