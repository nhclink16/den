import AVKit
import LiveKit
import Observation
import SwiftUI

/// Retain on the call owner, not in a sheet or a LazyVGrid cell.
/// Apple: https://developer.apple.com/documentation/avkit/adopting-picture-in-picture-for-video-calls
/// LiveKit 2.17.0: VideoView.renderMode = .sampleBuffer is the public renderer path.
/// This does not change capture options or the SDK's normal background camera suspension.
@MainActor @Observable final class CallPictureInPicture: NSObject {
    let isSupported = AVPictureInPictureController.isPictureInPictureSupported()
    private(set) var canStart = false
    private(set) var isActive = false
    private(set) var hasVideo = false
    private(set) var error: String?

    /// Open the existing call sheet. Complete restoration from that sheet's onAppear.
    @ObservationIgnored var onRestoreUserInterface: (@MainActor () -> Void)?
    @ObservationIgnored let sourceView = LiveKit.VideoView()
    @ObservationIgnored private var callID: UUID?
    @ObservationIgnored private var selectedTrack: VideoTrack?
    @ObservationIgnored private var controller: AVPictureInPictureController?
    @ObservationIgnored private var delegateProxy: CallPiPDelegate?
    @ObservationIgnored private var contentController: AVPictureInPictureVideoCallViewController?
    @ObservationIgnored private var contentVideoView: LiveKit.VideoView?
    @ObservationIgnored private var possibleObservation: NSKeyValueObservation?
    @ObservationIgnored private var restoration: CallPiPRestorationReply?
    @ObservationIgnored private var restoringCallID: UUID?

    override init() {
        super.init()
        sourceView.renderMode = .sampleBuffer
        sourceView.mirrorMode = .off
        sourceView.backgroundColor = .black
        sourceView.clipsToBounds = true
        sourceView.isUserInteractionEnabled = false
        sourceView.isAccessibilityElement = false
    }

    /// Invoke after the call's presentation snapshot changes, on the main actor.
    /// "Usable" means a subscribed, unmuted remote video track, in snapshot order.
    /// An empty/muted remote set stops video PiP; it never falls back to the local camera.
    func update(callID: UUID?, videos: [CallVideo]) {
        guard let callID else { end(); return }
        if self.callID != callID {
            end()
            self.callID = callID
        }
        guard isSupported,
              let video = videos.first(where: { !$0.isLocal && !$0.isMuted && $0.track != nil }),
              let track = video.track else {
            releaseVideo()
            return
        }

        selectedTrack = track // VideoView.track is weak; retain until this selection ends.
        sourceView.track = track
        sourceView.layoutMode = video.isShare ? .fit : .fill
        hasVideo = true
        if controller == nil { configureController() }
        contentVideoView?.track = track
        contentVideoView?.layoutMode = .fit
        if let dimensions = track.dimensions, dimensions.width > 0, dimensions.height > 0 {
            contentController?.preferredContentSize = CGSize(width: Int(dimensions.width), height: Int(dimensions.height))
        } else {
            contentController?.preferredContentSize = CGSize(width: 640, height: video.isShare ? 360 : 480)
        }
        refreshPossibility()
    }

    /// An explicit PiP button may call this; automatic background entry is owned by AVKit.
    func start() {
        guard callID != nil, hasVideo, sourceView.window != nil,
              let controller, controller.isPictureInPicturePossible else { return }
        error = nil
        controller.startPictureInPicture()
    }

    /// Stop PiP without leaving the call or discarding its eligible content source.
    func stop() { controller?.stopPictureInPicture() }

    /// Call synchronously before leave/disconnect, session replacement, or logout.
    /// Clearing the content source prevents a departed call from starting PiP later.
    func end() {
        callID = nil
        releaseVideo()
        error = nil
    }

    func clearError() { error = nil }

    /// Call from the actual call sheet's onAppear, not when merely setting its binding.
    func restoredUserInterface(for callID: UUID?) {
        guard callID != nil, callID == self.callID, callID == restoringCallID else {
            finishRestoration(false)
            return
        }
        finishRestoration(true)
    }

    private func configureController() {
        let content = AVPictureInPictureVideoCallViewController()
        content.loadViewIfNeeded()
        content.view.backgroundColor = .black
        let video = LiveKit.VideoView()
        video.renderMode = .sampleBuffer
        video.mirrorMode = .off
        video.layoutMode = .fit
        video.track = selectedTrack
        video.translatesAutoresizingMaskIntoConstraints = false
        content.view.addSubview(video)
        NSLayoutConstraint.activate([
            video.leadingAnchor.constraint(equalTo: content.view.leadingAnchor),
            video.trailingAnchor.constraint(equalTo: content.view.trailingAnchor),
            video.topAnchor.constraint(equalTo: content.view.topAnchor),
            video.bottomAnchor.constraint(equalTo: content.view.bottomAnchor),
        ])
        let source = AVPictureInPictureController.ContentSource(
            activeVideoCallSourceView: sourceView, contentViewController: content)
        let pip = AVPictureInPictureController(contentSource: source)
        let proxy = CallPiPDelegate(owner: self)
        pip.delegate = proxy
        pip.canStartPictureInPictureAutomaticallyFromInline = true
        contentController = content
        contentVideoView = video
        controller = pip
        delegateProxy = proxy
        possibleObservation = pip.observe(\.isPictureInPicturePossible, options: [.initial, .new]) { [weak self] _, _ in
            Task { @MainActor [weak self] in self?.refreshPossibility() }
        }
    }

    private func refreshPossibility() {
        canStart = callID != nil && hasVideo && (controller?.isPictureInPicturePossible ?? false)
    }

    private func releaseVideo() {
        finishRestoration(false)
        possibleObservation?.invalidate()
        possibleObservation = nil
        controller?.canStartPictureInPictureAutomaticallyFromInline = false
        controller?.stopPictureInPicture()
        controller?.contentSource = nil
        controller?.delegate = nil
        controller = nil
        delegateProxy = nil
        sourceView.track = nil
        contentVideoView?.track = nil
        contentVideoView?.removeFromSuperview()
        contentVideoView = nil
        contentController = nil
        selectedTrack = nil
        hasVideo = false
        canStart = false
        isActive = false
    }

    private func finishRestoration(_ restored: Bool) {
        let completion = restoration
        restoration = nil
        restoringCallID = nil
        completion?.call(restored)
    }

    fileprivate func receive(_ event: CallPiPEvent, generation: UUID) {
        // A fresh delegate generation prevents callbacks from a released call/controller
        // affecting its replacement, even if Objective-C reuses an object address.
        guard delegateProxy?.generation == generation else {
            if case .restore(let reply) = event { reply.call(false) }
            return
        }
        switch event {
        case .started:
            isActive = true
            error = nil
        case .stopped:
            isActive = false
            refreshPossibility()
        case .failed:
            isActive = false
            error = "Couldn't open Picture in Picture."
            refreshPossibility()
        case .restore(let reply):
            guard let callID, let onRestoreUserInterface else { reply.call(false); return }
            finishRestoration(false)
            restoration = reply
            restoringCallID = callID
            onRestoreUserInterface()
        }
    }
}

fileprivate enum CallPiPEvent: Sendable {
    case started, stopped, failed
    case restore(CallPiPRestorationReply)
}

/// AVKit's delegate is nonisolated in the SDK. Only Sendable events cross to the
/// main actor; no AVKit controller or error object crosses the callback boundary.
fileprivate final class CallPiPDelegate: NSObject, AVPictureInPictureControllerDelegate {
    let generation = UUID()
    private weak var owner: CallPictureInPicture?

    @MainActor init(owner: CallPictureInPicture) { self.owner = owner }

    private func send(_ event: CallPiPEvent) {
        let generation = generation
        Task { @MainActor [weak owner = owner] in
            guard let owner else {
                if case .restore(let reply) = event { reply.call(false) }
                return
            }
            owner.receive(event, generation: generation)
        }
    }

    func pictureInPictureControllerDidStartPictureInPicture(_ controller: AVPictureInPictureController) { send(.started) }
    func pictureInPictureControllerDidStopPictureInPicture(_ controller: AVPictureInPictureController) { send(.stopped) }
    func pictureInPictureController(_ controller: AVPictureInPictureController,
                                   failedToStartPictureInPictureWithError error: any Error) { send(.failed) }
    func pictureInPictureController(_ controller: AVPictureInPictureController,
                                   restoreUserInterfaceForPictureInPictureStopWithCompletionHandler completionHandler: @escaping (Bool) -> Void) {
        send(.restore(CallPiPRestorationReply(completionHandler)))
    }
}

/// AVKit's Objective-C completion is not annotated Sendable. Transfer it once;
/// both the completion and its once-only gate are accessed exclusively on MainActor.
fileprivate final class CallPiPRestorationReply: @unchecked Sendable {
    private let completion: (Bool) -> Void
    @MainActor private var completed = false

    init(_ completion: @escaping (Bool) -> Void) { self.completion = completion }

    @MainActor func call(_ restored: Bool) {
        guard !completed else { return }
        completed = true
        completion(restored)
    }
}

/// Mount once in the persistent call dock, e.g. .frame(width: 64, height: 44).
/// Removing a thumbnail does not own call/PiP teardown; the call owner's end() does.
struct CallPiPThumbnail: UIViewRepresentable {
    let owner: CallPictureInPicture

    func makeUIView(context: Context) -> LiveKit.VideoView { owner.sourceView }
    func updateUIView(_ view: LiveKit.VideoView, context: Context) { }
}
