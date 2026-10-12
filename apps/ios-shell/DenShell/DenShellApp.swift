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

    /// A `blob:` URL has the origin of the page that made it, written after the scheme.
    init?(_ url: URL) {
        if url.scheme?.lowercased() == "blob" {
            guard let inner = URL(string: String(url.absoluteString.dropFirst("blob:".count))) else { return nil }
            self.init(inner)
            return
        }
        guard let scheme = url.scheme, let host = url.host() else { return nil }
        self.init(scheme: scheme, host: host, port: url.port ?? 0)
    }

    @MainActor init?(_ origin: WKSecurityOrigin) {
        self.init(scheme: origin.protocol, host: origin.host, port: origin.port)
    }

    /// As `location.origin` writes it: the port only when it isn't the scheme's default.
    var serialized: String { "\(scheme)://\(host)" + (port == (scheme == "https" ? 443 : 80) ? "" : ":\(port)") }
}

@main struct DenShellApp: App {
    init() {
        #if DEBUG
        CallTestHooks.install()
        #endif
    }

    var body: some Scene {
        WindowGroup {
            DenWebView().ignoresSafeArea()
        }
    }
}

struct DenWebView: UIViewRepresentable {
    func makeCoordinator() -> Shell { Shell(den: Origin(startURL)!) }

    func makeUIView(context: Context) -> UIView {
        let config = WKWebViewConfiguration()
        config.allowsInlineMediaPlayback = true
        config.mediaTypesRequiringUserActionForPlayback = []
        // Keep the page's timers and sockets running in the background, or a call drops for a reason
        // that has nothing to do with web calls themselves.
        config.preferences.inactiveSchedulingPolicy = .none
        context.coordinator.calls.install(in: config.userContentController)
        let webView = WKWebView(frame: .zero, configuration: config)
        context.coordinator.calls.webView = webView
        webView.navigationDelegate = context.coordinator
        webView.uiDelegate = context.coordinator
        webView.allowsBackForwardNavigationGestures = true
        #if DEBUG
        webView.isInspectable = true
        #endif
        context.coordinator.watchCalls(webView)
        webView.load(URLRequest(url: startURL))

        // The web app does not pad for the notch or home indicator, so the page stays inside the safe area
        // and the strips around it take the page's theme color. WebKit handles the keyboard, as in Safari.
        let container = UIView()
        container.addSubview(webView)
        webView.translatesAutoresizingMaskIntoConstraints = false
        let safe = container.safeAreaLayoutGuide
        NSLayoutConstraint.activate([
            webView.topAnchor.constraint(equalTo: safe.topAnchor),
            webView.bottomAnchor.constraint(equalTo: safe.bottomAnchor),
            webView.leadingAnchor.constraint(equalTo: safe.leadingAnchor),
            webView.trailingAnchor.constraint(equalTo: safe.trailingAnchor),
        ])
        context.coordinator.themeObservation = webView.observe(\.themeColor, options: [.initial]) { [weak container] webView, _ in
            MainActor.assumeIsolated { container?.backgroundColor = webView.themeColor ?? .systemBackground }
        }
        return container
    }

    func updateUIView(_ container: UIView, context: Context) {}
}

@MainActor final class Shell: NSObject, WKNavigationDelegate, WKUIDelegate {
    let den: Origin
    let engine = CallEngine()
    lazy var calls = CallBridge(den: den, engine: engine)
    private var observations: [NSKeyValueObservation] = []
    var themeObservation: NSKeyValueObservation?
    private var inCall = false
    private weak var sheet: WKWebView?

    init(den: Origin) {
        self.den = den
        super.init()
        // A phone call or Siri switches the session off, and capture can stay on throughout,
        // so switch it back on when the interruption ends or the app comes back.
        for name in [AVAudioSession.interruptionNotification, UIApplication.didBecomeActiveNotification] {
            NotificationCenter.default.addObserver(forName: name, object: nil, queue: .main) { [weak self] note in
                let ended = note.name != AVAudioSession.interruptionNotification
                    || note.userInfo?[AVAudioSessionInterruptionTypeKey] as? UInt == AVAudioSession.InterruptionType.ended.rawValue
                MainActor.assumeIsolated { if ended, self?.inCall == true { self?.applyCallAudio() } }
            }
        }
    }

    // MARK: Camera and microphone

    func webView(
        _ webView: WKWebView, requestMediaCapturePermissionFor origin: WKSecurityOrigin,
        initiatedByFrame frame: WKFrameInfo, type: WKMediaCaptureType,
        decisionHandler: @escaping @MainActor @Sendable (WKPermissionDecision) -> Void
    ) {
        // Both the page and the asking frame must be Den: an embedded frame from elsewhere gets nothing.
        // During a native call the engine has the mic; a second capture from the page would fight it.
        let allowed = Origin(origin) == den && Origin(frame.securityOrigin) == den && !engine.inCall
        log.info("media capture \(allowed ? "granted" : "denied", privacy: .public) for \(frame.securityOrigin.host, privacy: .public):\(frame.securityOrigin.port, privacy: .public)")
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
        applyCallAudio()
    }

    private func applyCallAudio() {
        // CallKit owns the session during a native call.
        guard !engine.inCall else { return }
        let session = AVAudioSession.sharedInstance()
        do {
            if inCall {
                try session.setCategory(.playAndRecord, mode: .voiceChat, options: [.allowBluetoothHFP, .defaultToSpeaker])
                try session.setActive(true)
            } else {
                try session.setActive(false, options: .notifyOthersOnDeactivation)
            }
            log.info("call audio session \(self.inCall ? "on" : "off", privacy: .public)")
        } catch {
            log.error("call audio session \(self.inCall ? "on" : "off", privacy: .public) failed: \(error, privacy: .public)")
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
        if Origin(url) == den {
            if action.targetFrame == nil { openInSheet(action.request, from: webView); return decisionHandler(.cancel) }
            return decisionHandler(.allow)
        }
        // Other sites go to Safari, mailto: and tel: to their apps. A blank, data or foreign blob page
        // never replaces Den.
        if !["about", "blob", "data"].contains(url.scheme?.lowercased() ?? "") { UIApplication.shared.open(url) }
        decisionHandler(.cancel)
    }

    /// `window.open` from the page: same rule as links, never a second web view.
    func webView(
        _ webView: WKWebView, createWebViewWith configuration: WKWebViewConfiguration,
        for action: WKNavigationAction, windowFeatures: WKWindowFeatures
    ) -> WKWebView? {
        if let url = action.request.url, Origin(url) != nil {
            if Origin(url) == den { openInSheet(action.request, from: webView) } else { UIApplication.shared.open(url) }
        }
        return nil
    }

    /// A Den link meant for a new window (a picture's Open original) opens in a sheet over Den with a Done button,
    /// never in place of Den: the shell has no back button. The sheet shares Den's cookies and these rules.
    private func openInSheet(_ request: URLRequest, from webView: WKWebView) {
        if webView === sheet { webView.load(request); return }
        let viewer = WKWebView(frame: .zero, configuration: webView.configuration)
        viewer.navigationDelegate = self
        viewer.uiDelegate = self
        viewer.load(request)
        let page = UIViewController()
        page.view = viewer
        page.navigationItem.rightBarButtonItem = UIBarButtonItem(
            systemItem: .done, primaryAction: UIAction { [weak page] _ in page?.dismiss(animated: true) })
        var top = webView.window?.rootViewController
        while let presented = top?.presentedViewController { top = presented }
        top?.present(UINavigationController(rootViewController: page), animated: true)
        sheet = viewer
    }

    /// iOS can kill the page's process under memory pressure; reload rather than show a blank screen.
    func webViewWebContentProcessDidTerminate(_ webView: WKWebView) {
        log.error("web content process terminated, reloading")
        webView.reload()
    }
}
