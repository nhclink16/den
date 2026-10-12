import AVFoundation
import CallKit
import LiveKit
import OSLog

// The native call engine: the web page's Join button hands it a LiveKit token, and it runs the call
// as a CallKit call, so the call keeps going with the phone locked or in another app.
// Voice only for now: it never subscribes to video, and reports cameras and shares as flags.

private let log = Logger(subsystem: "app.denchat.shell", category: "call")

/// What the page needs to draw the call. Always sent whole, so a missed one is fixed by the next.
struct CallSnapshot: Encodable, Equatable {
    enum Phase: String, Encodable { case connecting, connected, reconnecting, ended }
    struct Person: Encodable, Equatable {
        let identity: String
        let name: String
        let local: Bool
        let speaking: Bool
        let muted: Bool
        let quality: String
        let camera: Bool
        let screen: Bool
    }
    var phase: Phase
    var channelId: String
    var micOn: Bool
    var soundOff: Bool
    var otherDevices: Int
    var error: String?
    var participants: [Person]
}

struct CallJoin: Equatable {
    let url: URL
    let token: String
    let channelId: String
    let title: String
    let micOn: Bool
}

struct CallError: LocalizedError {
    let message: String
    var errorDescription: String? { message }
    static let busy = CallError(message: "Leave the current call before joining another.")
    static let ended = CallError(message: "This call has ended.")
    static let microphone = CallError(message: "Allow microphone access for Den Shell in Settings to speak in calls.")
}

/// LiveKit identities are `accountId:session`, so one account on two devices has two identities.
enum CallRules {
    static func accountID(_ identity: String) -> String { String(identity.prefix { $0 != ":" }) }

    /// Already in this call on another device: join with mic and sound off, as the web does.
    static func joinsQuietly(localIdentity: String, remoteIdentities: [String]) -> Bool {
        remoteIdentities.contains { $0 != localIdentity && accountID($0) == accountID(localIdentity) }
    }

    /// Audio only, never your own other device's mic, nothing while sound is off.
    static func subscribes(isAudio: Bool, isMicrophone: Bool, identity: String, localIdentity: String, soundOff: Bool) -> Bool {
        isAudio && !soundOff && !(isMicrophone && accountID(identity) == accountID(localIdentity))
    }
}

@MainActor final class CallEngine: NSObject {
    var onChange: (@MainActor (CallSnapshot) -> Void)?

    private final class Call {
        let id = UUID()
        let join: CallJoin
        let room: Room
        var phase = CallSnapshot.Phase.connecting
        var wantsMic: Bool
        var soundOff = false
        var held = false
        var audioActive = false
        var reportedConnected = false
        var error: String?
        var subscribed: [String: Bool] = [:]
        /// Set once `finish` has torn the call down; `leave` waits for it.
        var finished = false
        var finishWaiters: [CheckedContinuation<Void, Never>] = []
        init(join: CallJoin, room: Room, wantsMic: Bool) { self.join = join; self.room = room; self.wantsMic = wantsMic }
    }

    private let provider: CXProvider
    private let system = CXCallController()
    // The simulator has no system call screen, so CallKit ends every call there about a second after it starts.
    // Debug simulator builds switch the audio session on themselves instead; every phone build uses CallKit.
    #if DEBUG && targetEnvironment(simulator)
    private let usesCallKit = false
    #else
    private let usesCallKit = true
    #endif
    private var call: Call?
    private var reconciling = false
    private var reconcileAgain = false

    var inCall: Bool { call != nil }

    override init() {
        // CallKit owns switching the audio session on and off. LiveKit only gets the engine once
        // CallKit has activated it (`applyAudioGate`).
        AudioManager.shared.audioSession.isAutomaticConfigurationEnabled = false
        do {
            try AudioManager.shared.setEngineAvailability(.none)
            try Self.prepareAudioSession()
        } catch {
            log.error("call audio preparation failed: \(error, privacy: .public)")
        }
        let configuration = CXProviderConfiguration()
        configuration.supportsVideo = false
        configuration.maximumCallGroups = 1
        configuration.maximumCallsPerCallGroup = 1
        configuration.supportedHandleTypes = [.generic]
        configuration.includesCallsInRecents = false
        provider = CXProvider(configuration: configuration)
        super.init()
        provider.setDelegate(self, queue: .main)
        NotificationCenter.default.addObserver(forName: AVAudioSession.routeChangeNotification, object: nil, queue: .main) { _ in
            let outputs = AVAudioSession.sharedInstance().currentRoute.outputs.map(\.portName).joined(separator: ", ")
            log.info("audio route: \(outputs, privacy: .public)")
        }
    }

    /// Speaker unless headphones or AirPods are in; iOS's call screen can switch to the earpiece.
    /// Set before CallKit starts, never activated here.
    static func prepareAudioSession() throws {
        try AVAudioSession.sharedInstance().setCategory(.playAndRecord, mode: .voiceChat, options: [.allowBluetoothHFP, .defaultToSpeaker])
    }

    var snapshot: CallSnapshot? { call.map(snapshot) }

    // MARK: Page commands

    func join(_ request: CallJoin) async throws -> CallSnapshot {
        if let call {
            guard call.join.channelId == request.channelId else { throw CallError.busy }
            return snapshot(call)
        }
        let room = Room(delegate: self, connectOptions: ConnectOptions(autoSubscribe: false),
                        roomOptions: RoomOptions(adaptiveStream: true, dynacast: true))
        let call = Call(join: request, room: room, wantsMic: request.micOn)
        // Registered before the permission prompt, so a Leave during it finds and cancels this call.
        self.call = call
        publish()
        do {
            if request.micOn, !(await microphoneAllowed()) { call.wantsMic = false; call.error = CallError.microphone.message }
            try require(call)
            try Self.prepareAudioSession()
            if usesCallKit {
                let start = CXStartCallAction(call: call.id, handle: CXHandle(type: .generic, value: request.title))
                try await system.request(CXTransaction(action: start))
            } else {
                #if DEBUG && targetEnvironment(simulator)
                try AVAudioSession.sharedInstance().setActive(true)
                call.audioActive = true
                log.info("simulator: call audio session on without CallKit")
                #endif
            }
            try require(call)
            do { try await room.connect(url: request.url.absoluteString, token: request.token) }
            catch let error as LiveKitError where error.type == .timedOut {
                // LiveKit Swift 2.17.0 can drop its answer to the server's first offer (it is sent while its
                // request queue is still suspended), and the join then times out. A second try goes through.
                try require(call)
                log.error("join timed out, trying once more")
                try await room.connect(url: request.url.absoluteString, token: request.token)
            }
            try require(call)
            let local = room.localParticipant.identity?.stringValue ?? ""
            if CallRules.joinsQuietly(localIdentity: local, remoteIdentities: room.remoteParticipants.keys.map(\.stringValue)) {
                call.soundOff = true; call.wantsMic = false
            }
            call.phase = .connected
            reportConnected(call)
            applyAudioGate()
            await reconcile()
            try require(call)
            log.info("joined call \(call.id, privacy: .public)")
            return snapshot(call)
        } catch {
            log.error("join failed: \(error, privacy: .public)")
            if self.call === call { await finish(call, reason: .failed) }
            throw error is CallError ? error : CallError(message: "The call could not connect. Try again.")
        }
    }

    func leave() async {
        guard let call else { return }
        guard usesCallKit else { await finish(call, reason: nil); return }
        do { try await system.request(CXTransaction(action: CXEndCallAction(call: call.id))) }
        catch {
            // CallKit no longer knows the call (it failed to start, or was reset): end it here.
            await finish(call, reason: nil)
        }
        // CallKit accepting the transaction is not the end yet: the end action tears the call down later.
        // Wait for that, so a Join right after Leave doesn't find the old call still here.
        if !call.finished { await withCheckedContinuation { call.finishWaiters.append($0) } }
    }

    /// Goes through CallKit, so iOS's own mute button agrees with Den's.
    /// The replies below describe the current call, never the one the command began with: a reply that arrives
    /// after Leave and a rejoin must not end the new call in the page.
    func setMuted(_ muted: Bool) async throws -> CallSnapshot? {
        guard let call else { throw CallError.ended }
        if !muted, !(await microphoneAllowed()) { throw CallError.microphone }
        guard self.call === call else { return snapshot }
        if usesCallKit { try await system.request(CXTransaction(action: CXSetMutedCallAction(call: call.id, muted: muted))) }
        else { setMicrophone(call, on: !muted) }
        return snapshot
    }

    func setSoundOff(_ off: Bool) async throws -> CallSnapshot? {
        guard let call else { throw CallError.ended }
        call.soundOff = off
        applyAudioGate()
        await reconcile()
        return snapshot
    }

    // MARK: State

    private func require(_ call: Call) throws {
        guard self.call === call, call.phase != .ended else { throw CallError.ended }
    }

    private func microphoneAllowed() async -> Bool {
        switch AVAudioApplication.shared.recordPermission {
        case .granted: true
        case .undetermined: await AVAudioApplication.requestRecordPermission()
        default: false
        }
    }

    private func setMicrophone(_ call: Call, on: Bool) {
        call.wantsMic = on
        applyAudioGate()
        Task { await reconcile() }
    }

    private func reportConnected(_ call: Call) {
        guard usesCallKit, !call.reportedConnected else { return }
        call.reportedConnected = true
        provider.reportOutgoingCall(with: call.id, connectedAt: nil)
    }

    /// LiveKit's audio engine runs only while CallKit has the session on, and only the halves wanted.
    private func applyAudioGate() {
        let input = call.map { $0.audioActive && $0.wantsMic && !$0.held } ?? false
        let output = call.map { $0.audioActive && !$0.soundOff && !$0.held } ?? false
        do { try AudioManager.shared.setEngineAvailability(.init(isInputAvailable: input, isOutputAvailable: output)) }
        catch { log.error("audio engine gate failed: \(error, privacy: .public)") }
    }

    /// Brings subscriptions and the mic in line with what's wanted. One pass at a time; a change during
    /// a pass runs another.
    private func reconcile() async {
        guard !reconciling else { reconcileAgain = true; return }
        reconciling = true
        defer { reconciling = false }
        repeat {
            reconcileAgain = false
            guard let call, call.phase != .ended, call.phase != .connecting else { return }
            let room = call.room
            let local = room.localParticipant.identity?.stringValue ?? ""
            do {
                for participant in room.remoteParticipants.values {
                    let identity = participant.identity?.stringValue ?? ""
                    for case let publication as RemoteTrackPublication in participant.trackPublications.values {
                        let wanted = CallRules.subscribes(isAudio: publication.kind == .audio, isMicrophone: publication.source == .microphone,
                                                          identity: identity, localIdentity: local, soundOff: call.soundOff)
                        let key = publication.sid.stringValue
                        if call.subscribed[key] != wanted {
                            try await publication.set(subscribed: wanted)
                            call.subscribed[key] = wanted
                        }
                    }
                }
                if room.localParticipant.isMicrophoneEnabled() != call.wantsMic {
                    try await room.localParticipant.setMicrophone(enabled: call.wantsMic)
                }
            } catch {
                // An old call's pass failing after a new call began: still run the pass the new call asked for.
                guard self.call === call else { continue }
                log.error("call media update failed: \(error, privacy: .public)")
                call.error = "Couldn't update call audio. Try that again."
            }
            publish()
        } while reconcileAgain
    }

    private func finish(_ call: Call, reason: CXCallEndedReason?) async {
        guard self.call === call else { return }
        self.call = nil
        call.phase = .ended
        if usesCallKit, let reason { provider.reportCall(with: call.id, endedAt: nil, reason: reason) }
        applyAudioGate()
        #if DEBUG && targetEnvironment(simulator)
        try? AVAudioSession.sharedInstance().setActive(false, options: .notifyOthersOnDeactivation)
        #endif
        call.room.remove(delegate: self)
        // Tell the page before the slow disconnect, so a call joined meanwhile can't be hidden by this one's end.
        onChange?(snapshot(call))
        await call.room.disconnect()
        log.info("call ended \(call.id, privacy: .public)")
        call.finished = true
        for waiter in call.finishWaiters { waiter.resume() }
        call.finishWaiters = []
    }

    private func publish() {
        if let call { onChange?(snapshot(call)) }
    }

    private func snapshot(_ call: Call) -> CallSnapshot {
        let room = call.room
        let local = room.localParticipant
        let remotes = room.remoteParticipants.values.sorted { ($0.identity?.stringValue ?? "") < ($1.identity?.stringValue ?? "") }
        let everyone: [Participant] = call.phase == .ended ? [] : [local] + remotes
        let people = everyone.compactMap { p -> CallSnapshot.Person? in
            guard let identity = p.identity?.stringValue else { return nil }
            let isLocal = p === local
            return .init(identity: identity, name: p.name ?? "", local: isLocal, speaking: p.isSpeaking,
                         muted: isLocal ? !call.wantsMic : !p.isMicrophoneEnabled(), quality: Self.quality(p.connectionQuality),
                         camera: p.isCameraEnabled(), screen: p.isScreenShareEnabled())
        }
        let ownAccount = CallRules.accountID(local.identity?.stringValue ?? "")
        return CallSnapshot(
            phase: call.phase, channelId: call.join.channelId, micOn: call.wantsMic, soundOff: call.soundOff,
            otherDevices: remotes.filter { CallRules.accountID($0.identity?.stringValue ?? "") == ownAccount && !ownAccount.isEmpty }.count,
            error: call.error, participants: people)
    }

    private static func quality(_ quality: ConnectionQuality) -> String {
        switch quality {
        case .excellent: "excellent"
        case .good: "good"
        case .poor: "poor"
        case .lost: "lost"
        default: "unknown"
        }
    }

    // MARK: Room events

    fileprivate func roomChanged(_ room: Room, publications: Bool) {
        guard let call, call.room === room else { return }
        if publications { Task { await reconcile() } } else { publish() }
    }

    fileprivate func roomReconnecting(_ room: Room, done: Bool) {
        guard let call, call.room === room, call.phase != .ended else { return }
        call.phase = done ? .connected : .reconnecting
        if done { call.subscribed = [:]; Task { await reconcile() } } else { publish() }
    }

    fileprivate func roomDisconnected(_ room: Room) {
        guard let call, call.room === room else { return }
        call.error = "The call disconnected."
        Task { await finish(call, reason: .failed) }
    }

    // MARK: CallKit

    fileprivate func perform(_ action: CXCallAction) {
        guard let call, call.id == action.callUUID else {
            // A call CallKit still knows that is no longer ours: let it go.
            if action is CXEndCallAction { action.fulfill() } else { action.fail() }
            return
        }
        switch action {
        case is CXStartCallAction:
            do { try Self.prepareAudioSession() } catch { log.error("call audio category failed: \(error, privacy: .public)") }
            provider.reportOutgoingCall(with: call.id, startedConnectingAt: nil)
            action.fulfill()
        case is CXEndCallAction:
            Task { await finish(call, reason: nil); action.fulfill() }
        case let mute as CXSetMutedCallAction:
            // From Den or from iOS's call screen.
            if !mute.isMuted, AVAudioApplication.shared.recordPermission != .granted { action.fail(); return }
            setMicrophone(call, on: !mute.isMuted)
            action.fulfill()
        case let hold as CXSetHeldCallAction:
            // A phone call took over with "Hold & Accept": mic and sound off until it's released.
            call.held = hold.isOnHold
            applyAudioGate()
            action.fulfill()
            publish()
        default:
            action.fail()
        }
    }

    fileprivate func audioSession(active: Bool) {
        log.info("call audio session \(active ? "activated" : "deactivated", privacy: .public) by CallKit")
        call?.audioActive = active
        applyAudioGate()
    }

    fileprivate func reset() {
        log.error("CallKit reset the provider")
        if let call { Task { await finish(call, reason: nil) } }
    }
}

extension CallEngine: @preconcurrency CXProviderDelegate {
    // The provider runs on the main queue.
    func providerDidReset(_ provider: CXProvider) { reset() }
    func provider(_ provider: CXProvider, perform action: CXStartCallAction) { perform(action) }
    func provider(_ provider: CXProvider, perform action: CXEndCallAction) { perform(action) }
    func provider(_ provider: CXProvider, perform action: CXSetMutedCallAction) { perform(action) }
    func provider(_ provider: CXProvider, perform action: CXSetHeldCallAction) { perform(action) }
    func provider(_ provider: CXProvider, didActivate audioSession: AVAudioSession) { self.audioSession(active: true) }
    func provider(_ provider: CXProvider, didDeactivate audioSession: AVAudioSession) { self.audioSession(active: false) }
}

extension CallEngine: RoomDelegate {
    nonisolated private func deliver(_ room: Room, _ change: @escaping @MainActor @Sendable (CallEngine) -> Void) {
        Task { @MainActor [weak self] in if let self { change(self) } }
    }
    nonisolated func room(_ room: Room, participantDidConnect participant: RemoteParticipant) { deliver(room) { $0.roomChanged(room, publications: true) } }
    nonisolated func room(_ room: Room, participantDidDisconnect participant: RemoteParticipant) { deliver(room) { $0.roomChanged(room, publications: false) } }
    nonisolated func room(_ room: Room, didUpdateSpeakingParticipants participants: [Participant]) { deliver(room) { $0.roomChanged(room, publications: false) } }
    nonisolated func room(_ room: Room, participant: Participant, didUpdateName name: String) { deliver(room) { $0.roomChanged(room, publications: false) } }
    nonisolated func room(_ room: Room, participant: Participant, didUpdateConnectionQuality quality: ConnectionQuality) { deliver(room) { $0.roomChanged(room, publications: false) } }
    nonisolated func room(_ room: Room, participant: Participant, trackPublication: TrackPublication, didUpdateIsMuted isMuted: Bool) { deliver(room) { $0.roomChanged(room, publications: false) } }
    nonisolated func room(_ room: Room, participant: LocalParticipant, didPublishTrack publication: LocalTrackPublication) { deliver(room) { $0.roomChanged(room, publications: false) } }
    nonisolated func room(_ room: Room, participant: LocalParticipant, didUnpublishTrack publication: LocalTrackPublication) { deliver(room) { $0.roomChanged(room, publications: false) } }
    nonisolated func room(_ room: Room, participant: RemoteParticipant, didPublishTrack publication: RemoteTrackPublication) { deliver(room) { $0.roomChanged(room, publications: true) } }
    nonisolated func room(_ room: Room, participant: RemoteParticipant, didUnpublishTrack publication: RemoteTrackPublication) { deliver(room) { $0.roomChanged(room, publications: false) } }
    nonisolated func room(_ room: Room, didStartReconnectWithMode reconnectMode: ReconnectMode) { deliver(room) { $0.roomReconnecting(room, done: false) } }
    nonisolated func room(_ room: Room, didCompleteReconnectWithMode reconnectMode: ReconnectMode) { deliver(room) { $0.roomReconnecting(room, done: true) } }
    nonisolated func room(_ room: Room, didDisconnectWithError error: LiveKitError?) { deliver(room) { $0.roomDisconnected(room) } }
}
