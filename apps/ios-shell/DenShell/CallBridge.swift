import OSLog
import UIKit
import WebKit

// The page ↔ call engine bridge. The page calls `window.denShell.call({type, …})`, which returns a promise;
// the engine answers through `window.denShell.onCall(snapshot)`. Only the Den origin's main frame may use it.

private let log = Logger(subsystem: "app.denchat.shell", category: "bridge")

/// One message from the page, checked before anything acts on it.
enum CallCommand: Equatable {
    case join(CallJoin)
    case leave
    case mute(Bool, seq: Int)
    case sound(off: Bool, seq: Int)
    case state

    /// `allowLocalWS`: plain `ws://` to this machine, for debug builds against a local LiveKit.
    init(_ body: Any, allowLocalWS: Bool) throws {
        guard let body = body as? [String: Any], let type = body["type"] as? String else { throw CallError(message: "Unreadable call message.") }
        func flag(_ key: String) throws -> Bool {
            // JavaScript booleans arrive as NSNumber; refuse plain numbers so 0/1 can't pass as a choice.
            guard let value = body[key] as? NSNumber, CFGetTypeID(value) == CFBooleanGetTypeID() else { throw CallError(message: "Missing \(key).") }
            return value.boolValue
        }
        func count(_ key: String) throws -> Int {
            guard let value = body[key] as? NSNumber, CFGetTypeID(value) != CFBooleanGetTypeID(), value.intValue >= 0,
                  Double(value.intValue) == value.doubleValue else { throw CallError(message: "Missing \(key).") }
            return value.intValue
        }
        func text(_ key: String) throws -> String {
            guard let value = body[key] as? String, !value.isEmpty else { throw CallError(message: "Missing \(key).") }
            return value
        }
        switch type {
        case "join":
            guard let url = URL(string: try text("url")), let host = url.host(), !host.isEmpty,
                  url.scheme == "wss" || (allowLocalWS && url.scheme == "ws" && ["localhost", "127.0.0.1"].contains(host))
            else { throw CallError(message: "The call server address isn't secure.") }
            self = .join(CallJoin(url: url, token: try text("token"), channelId: try text("channelId"),
                                  title: try text("title"), micOn: try flag("micOn")))
        case "leave": self = .leave
        case "mute": self = .mute(try flag("muted"), seq: try count("seq"))
        case "sound": self = .sound(off: try flag("off"), seq: try count("seq"))
        case "state": self = .state
        default: throw CallError(message: "Unknown call message.")
        }
    }
}

@MainActor final class CallBridge: NSObject, WKScriptMessageHandlerWithReply {
    let den: Origin
    let engine: CallEngine
    weak var webView: WKWebView?

    init(den: Origin, engine: CallEngine) {
        self.den = den
        self.engine = engine
        super.init()
        engine.onChange = { [weak self] in self?.send($0) }
        // The page may have slept through changes while the app was away, even a call's end. Catch it up on
        // return: "no call" is sent as a revision alone.
        NotificationCenter.default.addObserver(forName: UIApplication.didBecomeActiveNotification, object: nil, queue: .main) { [weak self] _ in
            MainActor.assumeIsolated { self?.send(self?.engine.snapshot) }
        }
    }

    func install(in controller: WKUserContentController) {
        // A JSON string literal is a valid JavaScript string literal.
        let origin = String(data: try! JSONEncoder().encode(den.serialized), encoding: .utf8)!
        let source = """
        if (location.origin === \(origin)) window.denShell = {
          call: m => window.webkit.messageHandlers.denCall.postMessage(m).then(r => r == null ? null : JSON.parse(r)),
          onCall: null,
        }
        """
        controller.addUserScript(WKUserScript(source: source, injectionTime: .atDocumentStart, forMainFrameOnly: true))
        controller.addScriptMessageHandler(self, contentWorld: .page, name: "denCall")
    }

    /// Whether a message may drive the call: the Den page itself, never a frame inside it.
    nonisolated static func allows(isMainFrame: Bool, origin: Origin?, den: Origin) -> Bool { isMainFrame && origin == den }

    func userContentController(_ controller: WKUserContentController, didReceive message: WKScriptMessage) async -> (Any?, String?) {
        guard Self.allows(isMainFrame: message.frameInfo.isMainFrame, origin: Origin(message.frameInfo.securityOrigin), den: den) else {
            log.error("call message refused from \(message.frameInfo.securityOrigin.host, privacy: .public)")
            return (nil, "Not allowed.")
        }
        #if DEBUG
        let allowLocalWS = true
        #else
        let allowLocalWS = false
        #endif
        do {
            let snapshot: CallSnapshot?
            switch try CallCommand(message.body, allowLocalWS: allowLocalWS) {
            case .join(let request): snapshot = try await engine.join(request)
            case .leave: await engine.leave(); snapshot = nil
            case .mute(let muted, let seq): snapshot = try await engine.setMuted(muted, seq: seq)
            case .sound(let off, let seq): snapshot = try await engine.setSoundOff(off, seq: seq)
            case .state: snapshot = engine.snapshot
            }
            return (try snapshot.map(Self.json) ?? engine.noCall(), nil)
        } catch {
            return (nil, error.localizedDescription)
        }
    }

    private func send(_ snapshot: CallSnapshot?) {
        guard let webView, let json = try? snapshot.map(Self.json) ?? engine.noCall() else { return }
        webView.callAsyncJavaScript("window.denShell?.onCall?.(JSON.parse(s))", arguments: ["s": json], in: nil, in: .page) { result in
            if case .failure(let error) = result { log.error("call snapshot not delivered: \(error, privacy: .public)") }
        }
    }

    static func json(_ snapshot: CallSnapshot) throws -> String {
        String(data: try JSONEncoder().encode(snapshot), encoding: .utf8)!
    }
}
