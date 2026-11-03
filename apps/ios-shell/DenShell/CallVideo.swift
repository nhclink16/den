import AVFoundation
import LiveKit
import OSLog
import UIKit
import WebKit

// Call video, drawn natively *behind* the web page. The page keeps drawing the call screen and leaves a see-through hole
// in each video tile (web: lib/shell-video.ts); the shell puts a LiveKit VideoView behind each hole. Whatever the page
// draws over a tile (the name, the talking ring, a sheet, a menu, a toast, another screen) is on top by itself, and a
// view that is a frame late moving can only show inside the tile: the page is opaque everywhere else.
//
// The page sends every slot each time (`video({type: 'slots', seq, slots})`): where each tile is, its corners, and
// whose camera or share it shows. The reply says which of them have their video drawn, once the views are on screen,
// and so does a push whenever that changes; both carry a growing `rev`, as they travel separately;
// the page opens a tile's hole only then, and closes it the moment the tile moves. Only slots on screen are sent, and
// the engine subscribes to just those videos, at the size their views ask for (LiveKit's adaptive stream).
//
// Your camera: `call({type: 'camera'})` publishes it (front camera first), `flip` turns it round. iOS stops the camera in
// the background, so it is paused then (others see it go off, not a frozen picture) and comes back on return; nobody's
// video is drawn or received while Den is in the background either.

private let log = Logger(subsystem: "app.denchat.shell", category: "video")

/// One video tile on the page, checked before anything acts on it.
struct VideoSlot: Equatable {
    enum Fit: String { case cover, contain }
    /// `CallRules.videoKey`: whose, which camera or share.
    let key: String
    /// The whole tile, in the web view's points; only the part the page leaves see-through shows.
    let rect: CGRect
    let radius: Double
    /// Round a fitted share, as the tile's own background.
    let fill: UIColor?
    let fit: Fit

    init(_ body: Any) throws {
        guard let body = body as? [String: Any], let key = body["key"] as? String, !key.isEmpty, key.count < 400,
              let rect = Self.rect(body["rect"]), let radius = Self.number(body["radius"]), radius >= 0,
              let fit = (body["fit"] as? String).flatMap(Fit.init) else { throw CallError(message: "Unreadable video slot.") }
        self.key = key
        self.rect = rect
        self.radius = min(radius, min(rect.width, rect.height) / 2)
        self.fill = (body["fill"] as? String).flatMap(Self.color)
        self.fit = fit
    }

    private static func number(_ value: Any?) -> Double? {
        guard let value = value as? NSNumber, CFGetTypeID(value) != CFBooleanGetTypeID(), value.doubleValue.isFinite else { return nil }
        return value.doubleValue
    }

    private static func rect(_ value: Any?) -> CGRect? {
        guard let rect = value as? [String: Any], let x = number(rect["x"]), let y = number(rect["y"]),
              let width = number(rect["width"]), let height = number(rect["height"]), width > 0, height > 0,
              max(abs(x), abs(y), width, height) < 1e5 else { return nil }
        return CGRect(x: x, y: y, width: width, height: height)
    }

    /// `rgb(r, g, b)` or `rgba(r, g, b, a)`, as a computed style gives a colour.
    static func color(_ css: String) -> UIColor? {
        guard css.hasPrefix("rgb"), let open = css.firstIndex(of: "("), css.hasSuffix(")") else { return nil }
        let parts = css[css.index(after: open) ..< css.index(before: css.endIndex)]
            .split(whereSeparator: { $0 == "," || $0 == " " || $0 == "/" }).compactMap { Double($0) }
        guard parts.count == 3 || parts.count == 4, parts.prefix(3).allSatisfy({ (0 ... 255).contains($0) }) else { return nil }
        return UIColor(red: parts[0] / 255, green: parts[1] / 255, blue: parts[2] / 255, alpha: parts.count == 4 ? min(max(parts[3], 0), 1) : 1)
    }
}

/// The page's command: every slot, and which placement this is.
struct VideoCommand: Equatable {
    let seq: Int
    let slots: [VideoSlot]

    init(_ body: Any) throws {
        guard let body = body as? [String: Any], body["type"] as? String == "slots", let seq = body["seq"] as? NSNumber,
              CFGetTypeID(seq) != CFBooleanGetTypeID(), seq.intValue >= 0, Double(seq.intValue) == seq.doubleValue,
              let slots = body["slots"] as? [Any], slots.count <= 32 else { throw CallError(message: "Unreadable video message.") }
        self.seq = seq.intValue
        self.slots = try slots.map(VideoSlot.init)
    }
}

/// One tile's video: a rounded view with LiveKit's renderer in it.
@MainActor final class VideoSlotView: UIView, VideoViewDelegate {
    let video = VideoView()
    /// A frame of the current track has been drawn.
    private(set) var drawn = false
    /// The UI check's test pattern stands in for a track (CallTestCall.swift).
    var patterned = false
    /// Its video is on screen: drawn, and still there. LiveKit holds the track weakly, so it can go by itself.
    var isReady: Bool { drawn && (video.track != nil || patterned) }
    var onDrawn: (() -> Void)?

    override init(frame: CGRect) {
        super.init(frame: frame)
        clipsToBounds = true
        isUserInteractionEnabled = false
        video.frame = bounds
        video.autoresizingMask = [.flexibleWidth, .flexibleHeight]
        // Your own front camera reads as a mirror, as it does on the web.
        video.mirrorMode = .auto
        video.delegates.add(delegate: self)
        addSubview(video)
    }

    @available(*, unavailable) required init?(coder: NSCoder) { fatalError() }

    var track: VideoTrack? {
        get { video.track }
        set {
            if newValue == nil || newValue !== video.track { drawn = false }
            video.track = newValue
        }
    }

    #if DEBUG && targetEnvironment(simulator)
    /// The UI check's test pattern counts as a drawn picture (CallTestCall.swift).
    func markDrawn() { patterned = true; if !drawn { drawn = true; onDrawn?() } }
    #endif

    nonisolated func videoView(_ videoView: VideoView, didUpdate isRendering: Bool) {
        guard isRendering else { return }
        Task { @MainActor in
            // Not undone when frames pause (a still share sends few): only a new track starts over. And only for the track
            // shown now: this may be the last word of the one before it.
            guard !self.drawn, self.video.track != nil, self.video.didRenderFirstFrame else { return }
            self.drawn = true
            self.onDrawn?()
        }
    }
}

@MainActor final class CallVideo: NSObject, WKScriptMessageHandlerWithReply {
    let den: Origin
    let engine: CallEngine
    weak var webView: WKWebView?
    /// Behind the web view, the same size: the slots are in its points.
    let layer = UIView()
    private var views: [String: VideoSlotView] = [:]
    private var seq = 0
    /// Grows with every word to the page, replies and pushes alike, so it can drop one that arrives late.
    private var rev = 0

    init(den: Origin, engine: CallEngine) {
        self.den = den
        self.engine = engine
        super.init()
        layer.isUserInteractionEnabled = false
        engine.onVideo = { [weak self] in self?.tracksChanged() }
        // Nothing is drawn while Den is in the background; the engine stops receiving too.
        for (name, shown) in [(UIApplication.didEnterBackgroundNotification, false), (UIApplication.willEnterForegroundNotification, true)] {
            NotificationCenter.default.addObserver(forName: name, object: nil, queue: .main) { [weak self] _ in
                MainActor.assumeIsolated { self?.views.values.forEach { $0.video.isEnabled = shown } }
            }
        }
    }

    /// Adds `video` and `onVideo` to the `window.denShell` CallBridge's script made, so install after it.
    func install(in controller: WKUserContentController) {
        let origin = String(data: try! JSONEncoder().encode(den.serialized), encoding: .utf8)!
        let source = """
        if (location.origin === \(origin) && window.denShell) {
          Object.assign(window.denShell, {
            video: c => window.webkit.messageHandlers.denVideo.postMessage(c).then(r => JSON.parse(r)),
            onVideo: null,
          })
        }
        """
        controller.addUserScript(WKUserScript(source: source, injectionTime: .atDocumentStart, forMainFrameOnly: true))
        controller.addScriptMessageHandler(self, contentWorld: .page, name: "denVideo")
    }

    /// Puts the video layer right behind the web view, over everything else behind it.
    func attach(behind webView: WKWebView) {
        guard let container = webView.superview else { return }
        self.webView = webView
        container.insertSubview(layer, belowSubview: webView)
        layer.translatesAutoresizingMaskIntoConstraints = false
        NSLayoutConstraint.activate([
            layer.topAnchor.constraint(equalTo: webView.topAnchor),
            layer.bottomAnchor.constraint(equalTo: webView.bottomAnchor),
            layer.leadingAnchor.constraint(equalTo: webView.leadingAnchor),
            layer.trailingAnchor.constraint(equalTo: webView.trailingAnchor),
        ])
    }

    func userContentController(_ controller: WKUserContentController, didReceive message: WKScriptMessage) async -> (Any?, String?) {
        guard CallBridge.allows(isMainFrame: message.frameInfo.isMainFrame, origin: Origin(message.frameInfo.securityOrigin), den: den) else {
            log.error("video message refused from \(message.frameInfo.securityOrigin.host, privacy: .public)")
            return (nil, "Not allowed.")
        }
        do {
            let command = try VideoCommand(message.body)
            place(command)
            // Answer once the views are on screen where the page said: it opens the holes then.
            try? await Task.sleep(for: .milliseconds(34))
            return (reply(seq: command.seq), nil)
        } catch {
            return (nil, error.localizedDescription)
        }
    }

    private func place(_ command: VideoCommand) {
        seq = command.seq
        let keys = Set(command.slots.map(\.key))
        for (key, view) in views where !keys.contains(key) {
            view.track = nil
            view.removeFromSuperview()
            views[key] = nil
        }
        let foreground = UIApplication.shared.applicationState != .background
        for slot in command.slots {
            let view: VideoSlotView
            if let shown = views[slot.key] { view = shown } else {
                view = VideoSlotView(frame: slot.rect)
                view.onDrawn = { [weak self] in self?.drawnChanged() }
                view.video.isEnabled = foreground
                layer.addSubview(view)
                views[slot.key] = view
            }
            view.frame = slot.rect
            view.layer.cornerRadius = slot.radius
            view.backgroundColor = slot.fill ?? .black
            view.video.layoutMode = slot.fit == .contain ? .fit : .fill
            #if DEBUG && targetEnvironment(simulator)
            if FakeCall.shared != nil { view.showTestPattern(slot.key); continue }
            #endif
            view.track = engine.videoTrack(slot.key)
        }
        engine.shownVideo = keys
    }

    /// A video came or went: each view takes whatever its slot names now.
    private func tracksChanged() {
        for (key, view) in views { view.track = engine.videoTrack(key) }
        drawnChanged()
    }

    private func drawnChanged() {
        rev += 1
        guard let webView, let json = String(data: (try? JSONSerialization.data(withJSONObject: ["type": "ready", "seq": seq, "rev": rev, "ready": ready])) ?? Data(), encoding: .utf8) else { return }
        webView.callAsyncJavaScript("window.denShell?.onVideo?.(JSON.parse(s))", arguments: ["s": json], in: nil, in: .page) { result in
            if case .failure(let error) = result { log.error("video event not delivered: \(error, privacy: .public)") }
        }
    }

    private var ready: [String] { views.filter { $0.value.isReady }.map(\.key).sorted() }

    private func reply(seq: Int) -> String {
        rev += 1
        return String(data: (try? JSONSerialization.data(withJSONObject: ["seq": seq, "rev": rev, "ready": ready])) ?? Data(), encoding: .utf8) ?? "{}"
    }

    /// The page went (a load, a crash): so do its videos.
    func pageGone() {
        for view in views.values { view.track = nil; view.removeFromSuperview() }
        views = [:]
        engine.shownVideo = []
    }
}

// MARK: The engine's side: whose video is which, and your camera.

extension CallError {
    static let camera = CallError(message: "Allow camera access for Den in Settings to share your camera.")
}

extension CallEngine {
    /// The video a slot names, local or remote, if there is one to draw.
    func videoTrack(_ key: String) -> VideoTrack? {
        guard let room = call?.room, call?.phase != .ended else { return nil }
        let everyone: [Participant] = [room.localParticipant] + Array(room.remoteParticipants.values)
        for participant in everyone {
            for publication in participant.trackPublications.values where publication.kind == .video && !publication.isMuted {
                if CallRules.videoKey(identity: participant.identity?.stringValue ?? "", source: publication.source, name: publication.name) == key {
                    return publication.track as? VideoTrack
                }
            }
        }
        return nil
    }

    /// Each screen share a person has on, by track name, with the label the web gave it.
    static func screens(_ participant: Participant) -> [CallSnapshot.Screen] {
        participant.trackPublications.values.filter { $0.source == .screenShareVideo && !$0.isMuted }.map(\.name).sorted().map {
            .init(name: $0, label: participant.attributes["den.share.\($0)"] ?? "", surface: participant.attributes["den.share-surface.\($0)"] ?? "")
        }
    }

    /// `seq` counts as applied once the camera is wanted or not (it may still be starting); on a refusal too.
    func setCamera(_ on: Bool, seq: Int) async throws -> CallSnapshot? {
        guard let call else { throw CallError.ended }
        if on, !(await Self.cameraAllowed()) {
            if self.call === call { call.cameraSeq = seq; publish() }
            throw CallError.camera
        }
        guard self.call === call else { return snapshot }
        call.cameraSeq = seq
        call.wantsCamera = on
        // Off at once, before the answer, even while another pass is busy (LiveKit queues its publish calls).
        if !on { await updateCamera(call) }
        await reconcile()
        return snapshot
    }

    /// In a pass like any other camera change, so it can't restart a camera that a pass meanwhile turned off.
    func flipCamera() async throws -> CallSnapshot? {
        guard let call else { throw CallError.ended }
        call.flip = true
        await reconcile()
        return snapshot
    }

    private static func cameraAllowed() async -> Bool {
        switch AVCaptureDevice.authorizationStatus(for: .video) {
        case .authorized: true
        case .notDetermined: await AVCaptureDevice.requestAccess(for: .video)
        default: false
        }
    }

    /// The camera runs while it's wanted and Den is on screen. One change at a time, each deciding when its turn comes,
    /// so whoever waits on it (an Off's answer) knows the camera is where it was asked to be.
    func updateCamera(_ call: Call) async {
        let previous = call.cameraWork
        let work = Task { @MainActor in
            await previous?.value
            await self.applyCamera(call)
        }
        call.cameraWork = work
        await work.value
    }

    /// One that won't start is let go, and the page hears why.
    private func applyCamera(_ call: Call) async {
        guard self.call === call else { return }
        let live = call.wantsCamera && foreground
        let local = call.room.localParticipant
        do {
            if local.isCameraEnabled() != live {
                if live, !local.trackPublications.values.contains(where: { $0.source == .camera }) {
                    let track = await LocalVideoTrack.createCameraTrack(options: CameraCaptureOptions(position: call.cameraPosition))
                    // Still this call, and still wanted? Leave or Off may have come while the track was made.
                    guard self.call === call, call.wantsCamera, foreground else { return }
                    call.cameraTrack = track
                    do { try await local.publish(videoTrack: track) } catch { try? await track.stop(); call.cameraTrack = nil; throw error }
                } else {
                    // Mutes or unmutes the one published.
                    try await local.setCamera(enabled: live)
                }
            }
            // Only while it runs: a flip asked for otherwise is dropped, as there's no camera to turn.
            if call.flip {
                call.flip = false
                if live, local.isCameraEnabled(), let capturer = (local.firstCameraVideoTrack as? LocalVideoTrack)?.capturer as? CameraCapturer {
                    // One that fails leaves the camera as it was.
                    do { try await capturer.switchCameraPosition() } catch { log.error("camera flip failed: \(error, privacy: .public)") }
                    call.cameraPosition = capturer.position
                }
            }
        } catch {
            guard self.call === call else { return }
            log.error("camera \(live ? "on" : "off", privacy: .public) failed: \(error, privacy: .public)")
            if live { call.wantsCamera = false; call.error = "Your camera couldn't start." }
        }
    }
}
