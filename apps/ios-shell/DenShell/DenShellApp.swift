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
        #if DEBUG && targetEnvironment(simulator)
        // Kept for launches iOS starts itself, such as a notification tap, which carry no launch arguments.
        UserDefaults.standard.set(value, forKey: "DenStartURL")
        #endif
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
    @UIApplicationDelegateAdaptor(AppDelegate.self) private var appDelegate
    @State private var unreachable = Unreachable()

    init() {
        #if DEBUG && targetEnvironment(simulator)
        CallTestHooks.install()
        #endif
    }

    var body: some Scene {
        WindowGroup {
            ZStack {
                DenWebView(unreachable: unreachable).ignoresSafeArea()
                if unreachable.shown { UnreachableView(model: unreachable) }
            }
        }
    }
}

struct DenWebView: UIViewRepresentable {
    let unreachable: Unreachable

    func makeCoordinator() -> Shell { Shell(den: Origin(startURL)!, unreachable: unreachable) }

    func makeUIView(context: Context) -> UIView {
        let config = WKWebViewConfiguration()
        config.allowsInlineMediaPlayback = true
        config.mediaTypesRequiringUserActionForPlayback = []
        // Keep the page's timers and sockets running in the background, or a call drops for a reason
        // that has nothing to do with web calls themselves.
        config.preferences.inactiveSchedulingPolicy = .none
        context.coordinator.calls.install(in: config.userContentController)
        PushBridge.shared.install(in: config.userContentController, den: context.coordinator.den)
        context.coordinator.haptics.install(in: config.userContentController)
        let webView = WKWebView(frame: .zero, configuration: config)
        context.coordinator.webView = webView
        context.coordinator.calls.webView = webView
        PushBridge.shared.webView = webView
        webView.navigationDelegate = context.coordinator
        webView.uiDelegate = context.coordinator
        webView.allowsBackForwardNavigationGestures = true
        #if DEBUG
        webView.isInspectable = true
        #endif
        context.coordinator.watchCalls(webView)
        webView.load(URLRequest(url: startURL))

        // Edge to edge, like a native app: the page pads for the notch and the home indicator itself
        // (`env(safe-area-inset-*)`), so sheets reach the bottom edge and lists scroll under the home indicator.
        // The keyboard shortens the page instead of sliding it up, as in iMessage: the top bar stays and the
        // composer sits on the keyboard. The page never scrolls as a whole, so nothing may move it.
        let container = UIView()
        container.addSubview(webView)
        webView.translatesAutoresizingMaskIntoConstraints = false
        container.keyboardLayoutGuide.usesBottomSafeArea = false
        NSLayoutConstraint.activate([
            webView.topAnchor.constraint(equalTo: container.topAnchor),
            webView.bottomAnchor.constraint(equalTo: container.keyboardLayoutGuide.topAnchor),
            webView.leadingAnchor.constraint(equalTo: container.leadingAnchor),
            webView.trailingAnchor.constraint(equalTo: container.trailingAnchor),
        ])
        let scroll = webView.scrollView
        scroll.contentInsetAdjustmentBehavior = .never
        scroll.isScrollEnabled = false
        scroll.bounces = false
        context.coordinator.offsetObservation = scroll.observe(\.contentOffset) { scroll, _ in
            MainActor.assumeIsolated {
                if scroll.contentOffset != .zero && scroll.zoomScale <= scroll.minimumZoomScale { scroll.contentOffset = .zero }
            }
        }
        hideFormBar(webView)
        // The launch screen's mark stays put until the page draws its own in the same spot (the screen's centre), so
        // it doesn't blink out in between. The web view is see-through until then; the page's background covers it.
        webView.isOpaque = false
        webView.backgroundColor = .clear
        scroll.backgroundColor = .clear
        let mark = UIImageView(image: UIImage(named: "LaunchMark"))
        mark.translatesAutoresizingMaskIntoConstraints = false
        container.insertSubview(mark, belowSubview: webView)
        NSLayoutConstraint.activate([
            mark.centerXAnchor.constraint(equalTo: container.centerXAnchor),
            mark.centerYAnchor.constraint(equalTo: container.centerYAnchor),
        ])
        // Painted until the page draws, and kept for the unreachable screen of a later launch.
        Self.wear(Unreachable.lastThemeColor, container: container, mark: mark)
        context.coordinator.themeObservation = webView.observe(\.themeColor, options: [.initial]) { [weak container, weak mark] webView, _ in
            MainActor.assumeIsolated {
                guard let color = webView.themeColor, let container, let mark else { return }
                Self.wear(color, container: container, mark: mark)
                Unreachable.lastThemeColor = color
            }
        }
        return container
    }

    /// The page's colour behind it, and the mark in the light or dark ink that reads on that colour.
    private static func wear(_ color: UIColor?, container: UIView, mark: UIImageView) {
        container.backgroundColor = color ?? UIColor(named: "LaunchBackground")
        var (r, g, b, a): (CGFloat, CGFloat, CGFloat, CGFloat) = (0, 0, 0, 0)
        guard let color, color.getRed(&r, green: &g, blue: &b, alpha: &a) else { return }
        mark.overrideUserInterfaceStyle = 0.2126 * r + 0.7152 * g + 0.0722 * b < 0.5 ? .dark : .light
    }

    func updateUIView(_ container: UIView, context: Context) {}

    /// Safari's form bar (arrows and a tick above the keyboard) has no switch, and no app has one. The page's
    /// content view gets a subclass whose `inputAccessoryView` is nil, as Capacitor's keyboard plugin does.
    private func hideFormBar(_ webView: WKWebView) {
        guard let content = webView.scrollView.subviews.first(where: { NSStringFromClass(type(of: $0)).hasPrefix("WKContent") }),
              let base = object_getClass(content) else { return log.error("form bar: no content view") }
        let name = "DenNoFormBar_" + NSStringFromClass(base)
        var subclass: AnyClass? = NSClassFromString(name)
        if subclass == nil, let made = objc_allocateClassPair(base, name, 0) {
            let nothing: @convention(block) (AnyObject) -> UIView? = { _ in nil }
            class_addMethod(made, #selector(getter: UIResponder.inputAccessoryView), imp_implementationWithBlock(nothing), "@@:")
            objc_registerClassPair(made)
            subclass = made
        }
        if let subclass { object_setClass(content, subclass) }
    }
}

@MainActor final class Shell: NSObject, WKNavigationDelegate, WKUIDelegate {
    let den: Origin
    let unreachable: Unreachable
    weak var webView: WKWebView?
    let engine = CallEngine()
    lazy var calls = CallBridge(den: den, engine: engine)
    lazy var haptics = Haptics(den: den)
    private var observations: [NSKeyValueObservation] = []
    var themeObservation: NSKeyValueObservation?
    var offsetObservation: NSKeyValueObservation?
    private var retryTimer: Timer?
    private var inCall = false
    private weak var sheet: WKWebView?

    init(den: Origin, unreachable: Unreachable) {
        self.den = den
        self.unreachable = unreachable
        super.init()
        unreachable.retry = { [weak self] in self?.retry() }
        // A phone call or Siri switches the session off, and capture can stay on throughout,
        // so switch it back on when the interruption ends or the app comes back.
        for name in [AVAudioSession.interruptionNotification, UIApplication.didBecomeActiveNotification] {
            NotificationCenter.default.addObserver(forName: name, object: nil, queue: .main) { [weak self] note in
                let ended = note.name != AVAudioSession.interruptionNotification
                    || note.userInfo?[AVAudioSessionInterruptionTypeKey] as? UInt == AVAudioSession.InterruptionType.ended.rawValue
                let active = note.name == UIApplication.didBecomeActiveNotification
                MainActor.assumeIsolated {
                    if ended, self?.inCall == true { self?.applyCallAudio() }
                    if active, self?.unreachable.shown == true { self?.retry() }
                }
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
                // Den's haptics, which iOS otherwise mutes while the mic records.
                try session.setAllowHapticsAndSystemSoundsDuringRecording(true)
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

    // MARK: Unreachable server

    /// A Den that doesn't answer, or answers with a server error, gets the unreachable screen instead of a blank page.
    func webView(
        _ webView: WKWebView, decidePolicyFor response: WKNavigationResponse,
        decisionHandler: @escaping @MainActor @Sendable (WKNavigationResponsePolicy) -> Void
    ) {
        if webView === self.webView, response.isForMainFrame, let http = response.response as? HTTPURLResponse, http.statusCode >= 500 {
            log.error("den answered \(http.statusCode, privacy: .public)")
            showUnreachable(http.url)
            return decisionHandler(.cancel)
        }
        decisionHandler(.allow)
    }

    func webView(_ webView: WKWebView, didFailProvisionalNavigation navigation: WKNavigation!, withError error: any Error) {
        let error = error as NSError
        // A cancelled load (a newer one replaced it, or the policy above stopped it) isn't a failure.
        guard webView === self.webView, error.code != NSURLErrorCancelled, error.domain != "WebKitErrorDomain" else {
            if webView === self.webView { unreachable.trying = false }
            return
        }
        log.error("den unreachable: \(error.localizedDescription, privacy: .public)")
        showUnreachable(error.userInfo[NSURLErrorFailingURLErrorKey] as? URL)
    }

    /// A retry that got an answer and then lost the connection: let the next one go.
    func webView(_ webView: WKWebView, didFail navigation: WKNavigation!, withError error: any Error) {
        if webView === self.webView, unreachable.shown { unreachable.trying = false }
    }

    func webView(_ webView: WKWebView, didFinish navigation: WKNavigation!) {
        guard webView === self.webView, unreachable.shown else { return }
        retryTimer?.invalidate()
        unreachable.shown = false
        unreachable.trying = false
    }

    private func showUnreachable(_ url: URL?) {
        unreachable.url = url ?? unreachable.url ?? startURL
        unreachable.trying = false
        unreachable.shown = true
        // It keeps trying on its own, so a server that comes back opens by itself.
        guard retryTimer?.isValid != true else { return }
        retryTimer = Timer.scheduledTimer(withTimeInterval: 5, repeats: true) { [weak self] _ in
            MainActor.assumeIsolated { self?.retry() }
        }
    }

    private func retry() {
        guard let webView, unreachable.shown, !unreachable.trying else { return }
        unreachable.trying = true
        webView.load(URLRequest(url: unreachable.url ?? startURL))
    }

    /// iOS can kill the page's process under memory pressure; reload rather than show a blank screen.
    func webViewWebContentProcessDidTerminate(_ webView: WKWebView) {
        log.error("web content process terminated, reloading")
        webView.reload()
    }
}

// MARK: Unreachable server

/// What the unreachable screen shows. The shell fills it in; the screen only reads it and asks for a retry.
@MainActor @Observable final class Unreachable {
    var shown = false
    var trying = false
    var url: URL?
    @ObservationIgnored var retry: (@MainActor () -> Void)?

    /// The server as people know it: its host, and the port when it isn't the usual one.
    var server: String {
        let url = url ?? startURL
        guard let host = url.host() else { return "The server" }
        return url.port.map { "\(host):\($0)" } ?? host
    }

    /// The page's last theme colour, so the screen (and the first paint of a launch) wears Den's look, not white.
    static var lastThemeColor: UIColor? {
        get {
            guard let rgba = UserDefaults.standard.array(forKey: "DenThemeColor") as? [Double], rgba.count == 4 else { return nil }
            return UIColor(red: rgba[0], green: rgba[1], blue: rgba[2], alpha: rgba[3])
        }
        set {
            var (r, g, b, a): (CGFloat, CGFloat, CGFloat, CGFloat) = (0, 0, 0, 0)
            guard let newValue, newValue.getRed(&r, green: &g, blue: &b, alpha: &a) else { return }
            UserDefaults.standard.set([r, g, b, a].map(Double.init), forKey: "DenThemeColor")
        }
    }
}

/// Den couldn't load: the mark, what's wrong, and a way to try again, in the theme Den last wore.
struct UnreachableView: View {
    let model: Unreachable

    var body: some View {
        let background = Unreachable.lastThemeColor ?? .systemBackground
        var (r, g, b, a): (CGFloat, CGFloat, CGFloat, CGFloat) = (0, 0, 0, 0)
        let dark = background.resolvedColor(with: .current).getRed(&r, green: &g, blue: &b, alpha: &a) && 0.2126 * r + 0.7152 * g + 0.0722 * b < 0.5
        let ink = dark ? Color(red: 0.93, green: 0.9, blue: 0.85) : Color(red: 0.11, green: 0.1, blue: 0.09)
        return VStack(spacing: 0) {
            ZStack {
                DenMark.letter.fill(ink, style: FillStyle(eoFill: true))
                DenMark.door.fill(Color(red: 0.91, green: 0.64, blue: 0.29))
            }
            .frame(width: 64, height: 64)
            .accessibilityHidden(true)
            Text("Can't reach Den")
                .font(.title2.bold())
                .foregroundStyle(ink)
                .padding(.top, 24)
            Text("\(model.server) isn't answering. Den keeps trying on its own.")
                .font(.subheadline)
                .foregroundStyle(ink.opacity(0.6))
                .multilineTextAlignment(.center)
                .padding(.top, 8)
            Button { model.retry?() } label: {
                ZStack {
                    Text("Try Again").opacity(model.trying ? 0 : 1)
                    if model.trying { ProgressView().tint(Color(background)) }
                }
                .font(.body.weight(.semibold))
                .frame(minWidth: 180, minHeight: 50)
                .foregroundStyle(Color(background))
                .background(ink, in: Capsule())
            }
            .disabled(model.trying)
            .accessibilityLabel(model.trying ? "Trying again" : "Try Again")
            .padding(.top, 32)
        }
        .padding(32)
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .background(Color(background).ignoresSafeArea())
        .preferredColorScheme(dark ? .dark : .light)
    }
}

/// The Den mark (web: ui/Mark.svelte) in a 64-point box: the letter with its doorway cut out, and the lit door.
enum DenMark {
    static let letter = Path { p in
        p.move(to: CGPoint(x: 44, y: 4))
        p.addLine(to: CGPoint(x: 56, y: 4))
        p.addLine(to: CGPoint(x: 56, y: 56))
        p.addLine(to: CGPoint(x: 30, y: 56))
        p.addCurve(to: CGPoint(x: 8, y: 36), control1: CGPoint(x: 16, y: 56), control2: CGPoint(x: 8, y: 48))
        p.addCurve(to: CGPoint(x: 30, y: 14), control1: CGPoint(x: 8, y: 24), control2: CGPoint(x: 17, y: 14))
        p.addCurve(to: CGPoint(x: 44, y: 19), control1: CGPoint(x: 36, y: 14), control2: CGPoint(x: 40, y: 16))
        p.closeSubpath()
        p.addPath(arch(halfWidth: 10))
    }
    static let door = arch(halfWidth: 6)

    private static func arch(halfWidth w: CGFloat) -> Path {
        Path { p in
            p.move(to: CGPoint(x: 32 - w, y: 56))
            p.addLine(to: CGPoint(x: 32 - w, y: 36))
            p.addArc(center: CGPoint(x: 32, y: 36), radius: w, startAngle: .degrees(180), endAngle: .degrees(360), clockwise: false)
            p.addLine(to: CGPoint(x: 32 + w, y: 56))
            p.closeSubpath()
        }
    }
}
