import OSLog
import UIKit
import WebKit

// Taps the page asks for at a few moments (a sent message, joining a call), through `window.denShell.haptic(kind)`,
// next to `call` and `push`. Fire and forget: nothing comes back to the page, so a refusal is only logged. iOS skips
// them when the person turned System Haptics off.

private let log = Logger(subsystem: "app.denchat.shell", category: "haptics")

/// The few kinds the page may ask for. Anything else is refused.
enum Haptic: String, CaseIterable {
    case light, medium, selection, warning

    init?(_ body: Any) {
        guard let name = body as? String else { return nil }
        self.init(rawValue: name)
    }
}

@MainActor final class Haptics: NSObject, WKScriptMessageHandler {
    let den: Origin
    private lazy var light = UIImpactFeedbackGenerator(style: .light)
    private lazy var medium = UIImpactFeedbackGenerator(style: .medium)
    private lazy var selection = UISelectionFeedbackGenerator()
    private lazy var notification = UINotificationFeedbackGenerator()

    init(den: Origin) {
        self.den = den
        super.init()
    }

    /// Adds `haptic` to the `window.denShell` that CallBridge's script made, so install after it.
    func install(in controller: WKUserContentController) {
        // A JSON string literal is a valid JavaScript string literal.
        let origin = String(data: try! JSONEncoder().encode(den.serialized), encoding: .utf8)!
        let source = """
        if (location.origin === \(origin) && window.denShell) Object.assign(window.denShell, {
          haptic: k => { window.webkit.messageHandlers.denHaptic.postMessage(k) },
        })
        """
        controller.addUserScript(WKUserScript(source: source, injectionTime: .atDocumentStart, forMainFrameOnly: true))
        controller.addScriptMessageHandler(self, contentWorld: .page, name: "denHaptic")
    }

    func userContentController(_ controller: WKUserContentController, didReceive message: WKScriptMessage) {
        guard CallBridge.allows(isMainFrame: message.frameInfo.isMainFrame, origin: Origin(message.frameInfo.securityOrigin), den: den) else {
            log.error("haptic refused from \(message.frameInfo.securityOrigin.host, privacy: .public)")
            return
        }
        guard let kind = Haptic(message.body) else {
            log.error("unknown haptic refused")
            return
        }
        log.info("haptic \(kind.rawValue, privacy: .public)")
        switch kind {
        case .light: light.impactOccurred()
        case .medium: medium.impactOccurred()
        case .selection: selection.selectionChanged()
        case .warning: notification.notificationOccurred(.warning)
        }
    }
}
