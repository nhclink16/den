import AVFoundation
import OSLog
import SwiftUI
import WebKit

// A bare WKWebView around the Den web app, built for the M5 call test (docs/ROADMAP.md).
// It does only what a web call needs to get a fair chance on an iPhone.

private let log = Logger(subsystem: "app.denchat.shell", category: "shell")

/// The page the shell opens. Launch with `-DenStartURL https://host:port` to point it at a test server.
let startURL: URL = {
    if let value = UserDefaults.standard.string(forKey: "DenStartURL"), let url = URL(string: value), Origin(url) != nil {
        return url
    }
    return URL(string: "https://denchat.app")!
}()

/// Scheme, host and port, compared exactly. Only http and https have one.
struct Origin: Equatable {
    let scheme: String
    let host: String
    let port: Int

    /// `port` 0 means the scheme's default, which is how `WKSecurityOrigin` reports it.
    init?(scheme: String, host: String, port: Int) {
        let scheme = scheme.lowercased()
        let defaultPort = ["http": 80, "https": 443][scheme]
        guard let defaultPort, !host.isEmpty else { return nil }
        self.scheme = scheme
        self.host = host.lowercased()
        self.port = port == 0 ? defaultPort : port
    }

    init?(_ url: URL) {
        guard let scheme = url.scheme, let host = url.host() else { return nil }
        self.init(scheme: scheme, host: host, port: url.port ?? 0)
    }

    @MainActor init?(_ origin: WKSecurityOrigin) {
        self.init(scheme: origin.protocol, host: origin.host, port: origin.port)
    }
}

@main struct DenShellApp: App {
    var body: some Scene {
        WindowGroup {
            DenWebView().ignoresSafeArea()
        }
    }
}

struct DenWebView: UIViewRepresentable {
    func makeCoordinator() -> Shell { Shell(den: Origin(startURL)!) }

    func makeUIView(context: Context) -> WKWebView {
        let config = WKWebViewConfiguration()
        config.allowsInlineMediaPlayback = true
        config.mediaTypesRequiringUserActionForPlayback = []
        // Keep the page's timers and sockets running in the background, or a call drops for a reason
        // that has nothing to do with web calls themselves.
        config.preferences.inactiveSchedulingPolicy = .none
        let webView = WKWebView(frame: .zero, configuration: config)
        webView.navigationDelegate = context.coordinator
        webView.uiDelegate = context.coordinator
        webView.allowsBackForwardNavigationGestures = true
        #if DEBUG
        webView.isInspectable = true
        #endif
        context.coordinator.watchCalls(webView)
        webView.load(URLRequest(url: startURL))
        return webView
    }

    func updateUIView(_ webView: WKWebView, context: Context) {}
}

@MainActor final class Shell: NSObject, WKNavigationDelegate, WKUIDelegate {
    let den: Origin
    private var observations: [NSKeyValueObservation] = []
    private var inCall = false

    init(den: Origin) { self.den = den }

    // MARK: Camera and microphone

    func webView(
        _ webView: WKWebView, requestMediaCapturePermissionFor origin: WKSecurityOrigin,
        initiatedByFrame frame: WKFrameInfo, type: WKMediaCaptureType,
        decisionHandler: @escaping @MainActor @Sendable (WKPermissionDecision) -> Void
    ) {
        // Both the page and the asking frame must be Den: an embedded frame from elsewhere gets nothing.
        let allowed = Origin(origin) == den && Origin(frame.securityOrigin) == den
        decisionHandler(allowed ? .grant : .deny)
    }

    /// Treat the page as in a call while it captures the mic or camera, and give it a voice-chat audio session.
    func watchCalls(_ webView: WKWebView) {
        // WebKit reports both on the main thread.
        observations = [\WKWebView.microphoneCaptureState, \.cameraCaptureState].map { keyPath in
            webView.observe(keyPath) { [weak self] webView, _ in
                MainActor.assumeIsolated { self?.captureChanged(webView) }
            }
        }
    }

    private func captureChanged(_ webView: WKWebView) {
        let capturing = webView.microphoneCaptureState != .none || webView.cameraCaptureState != .none
        guard capturing != inCall else { return }
        inCall = capturing
        let session = AVAudioSession.sharedInstance()
        do {
            if capturing {
                try session.setCategory(.playAndRecord, mode: .voiceChat, options: [.allowBluetoothHFP, .defaultToSpeaker])
                try session.setActive(true)
            } else {
                try session.setActive(false, options: .notifyOthersOnDeactivation)
            }
            log.info("call audio session \(capturing ? "on" : "off", privacy: .public)")
        } catch {
            log.error("call audio session \(capturing ? "on" : "off", privacy: .public) failed: \(error, privacy: .public)")
        }
    }

    // MARK: Links

    func webView(
        _ webView: WKWebView, decidePolicyFor action: WKNavigationAction,
        decisionHandler: @escaping @MainActor @Sendable (WKNavigationActionPolicy) -> Void
    ) {
        // Frames inside the page (embeds, previews) load where they are.
        if let frame = action.targetFrame, !frame.isMainFrame { return decisionHandler(.allow) }
        guard let url = action.request.url else { return decisionHandler(.cancel) }
        if ["about", "blob", "data"].contains(url.scheme?.lowercased() ?? "") { return decisionHandler(.allow) }
        if Origin(url) == den {
            // A Den link meant for a new window stays in the shell.
            if action.targetFrame == nil { webView.load(action.request); return decisionHandler(.cancel) }
            return decisionHandler(.allow)
        }
        UIApplication.shared.open(url)
        decisionHandler(.cancel)
    }

    /// `window.open` from the page: same rule as links, never a second web view.
    func webView(
        _ webView: WKWebView, createWebViewWith configuration: WKWebViewConfiguration,
        for action: WKNavigationAction, windowFeatures: WKWindowFeatures
    ) -> WKWebView? {
        if let url = action.request.url, Origin(url) != nil {
            if Origin(url) == den { webView.load(action.request) } else { UIApplication.shared.open(url) }
        }
        return nil
    }

    /// iOS can kill the page's process under memory pressure; reload rather than show a blank screen.
    func webViewWebContentProcessDidTerminate(_ webView: WKWebView) {
        log.error("web content process terminated, reloading")
        webView.reload()
    }
}
