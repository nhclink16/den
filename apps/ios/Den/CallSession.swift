import AVFoundation
import DenAPI
import LiveKit
import Observation
import UIKit

@MainActor @Observable final class CallSession: NSObject {
    let pictureInPicture = CallPictureInPicture()
    enum Phase { case idle, connecting, connected, reconnecting, ending, failed }
    enum Route: Equatable { case system, speaker, input(String) }
    enum Failure: LocalizedError {
        case alreadyCalling, ended, microphonePermission, cameraPermission, waitingForAudio, media
        var errorDescription: String? {
            switch self {
            case .alreadyCalling: "Leave the current call before joining another."
            case .ended: "This call has ended."
            case .microphonePermission: "Allow microphone access in Settings to speak in this call."
            case .cameraPermission: "Allow camera access in Settings to share your camera."
            case .waitingForAudio: "Call audio is waiting for the system."
            case .media: "Couldn't update call media. Try that control again."
            }
        }
    }

    private(set) var callID: UUID?
    private(set) var channelID: String?
    private(set) var title = "Call"
    private(set) var phase = Phase.idle
    private(set) var microphoneEnabled = false
    private(set) var cameraEnabled = false
    private(set) var wantsMicrophone = false
    private(set) var wantsCamera = false
    private(set) var outputMuted = true
    private(set) var audioActivated = false
    private(set) var joinedQuietly = false
    private(set) var otherDevices = 0
    private(set) var people: [CallPerson] = []
    private(set) var videos: [CallVideo] = []
    private(set) var inputs: [CallAudioInput] = []
    private(set) var routeLabel = "System audio"
    private(set) var selectedRoute = Route.system
    var error: String?
    @ObservationIgnored var onUnexpectedDisconnect: (@MainActor (UUID) -> Void)?

    @ObservationIgnored private var room: Room?
    @ObservationIgnored private var accountID = ""
    @ObservationIgnored private var names: [String: String] = [:]
    @ObservationIgnored private var revision = 0
    @ObservationIgnored private var reconcileTask: Task<Void, Error>?
    @ObservationIgnored private var subscriptions: [String: Bool] = [:]
    @ObservationIgnored private var routeObserver: (any NSObjectProtocol)?

    var isActive: Bool { callID != nil && phase != .ending }
    var isConnected: Bool { phase == .connected || phase == .reconnecting }
    var status: String {
        switch phase {
        case .idle: "Not in a call"
        case .connecting: "Connecting"
        case .reconnecting: "Reconnecting"
        case .ending: "Leaving call"
        case .failed: "Call disconnected"
        case .connected: audioActivated ? "Connected" : "Waiting for call audio"
        }
    }

    /// CallKit owns activation. Joining never activates AVAudioSession itself.
    func join(callID: UUID, channelID: String, title: String, accountID: String,
              service: DenService, names: [String: String], initialMicrophone: Bool = true) async throws {
        guard self.callID == nil, phase != .ending else { throw Failure.alreadyCalling }
        self.callID = callID; self.channelID = channelID; self.title = title
        self.accountID = accountID; self.names = names
        phase = .connecting; error = nil; selectedRoute = .system
        wantsMicrophone = false; wantsCamera = false; outputMuted = true; audioActivated = false
        joinedQuietly = false; subscriptions = [:]
        AudioManager.shared.audioSession.isAutomaticConfigurationEnabled = false
        do {
            try AudioManager.shared.setEngineAvailability(.none)
            observeRoutes()
            let credential = try await service.client.postCallsChannelIdToken(path: .init(channelId: channelID)).ok.body.json
            try require(callID)
            let connection = Room(delegate: self,
                connectOptions: ConnectOptions(autoSubscribe: false, enableMicrophone: false),
                roomOptions: RoomOptions(adaptiveStream: true, dynacast: true))
            room = connection
            try await connection.connect(url: credential.url, token: credential.token)
            try require(callID)
            joinedQuietly = CallPolicy.joinsQuietly(accountID: accountID,
                localIdentity: connection.localParticipant.identity?.stringValue ?? "",
                remoteIdentities: connection.remoteParticipants.keys.map(\.stringValue))
            outputMuted = joinedQuietly
            // A locked incoming answer cannot display a first-use permission prompt.
            // Parent requests permission in a foreground action before the first call.
            wantsMicrophone = initialMicrophone && !joinedQuietly && AVCaptureDevice.authorizationStatus(for: .audio) == .authorized
            phase = .connected
            try applyAudioGate()
            refreshSnapshot(connection)
            revision += 1
            try await reconcile()
        } catch {
            if self.callID == callID {
                await leave(for: callID)
                self.error = error is CancellationError ? nil : "Couldn't connect to this call."
            }
            throw error
        }
    }

    /// Prepare the session before CXProvider registers it, and again before action fulfillment.
    /// Setting the category does not activate the session or open the LiveKit engine gate.
    static func prepareAudioSessionForCallKit() throws {
        try AVAudioSession.sharedInstance().setCategory(.playAndRecord, mode: .voiceChat, options: [.allowBluetoothHFP])
    }

    /// Use immediately before fulfilling CallKit's start/answer action. No setActive call.
    func configureAudioSession(for id: UUID) throws {
        try require(id)
        try Self.prepareAudioSessionForCallKit()
    }

    func audioSessionDidActivate(for id: UUID) throws {
        try require(id)
        audioActivated = true
        do { try applyAudioGate() }
        catch { audioActivated = false; try? applyAudioGate(); throw error }
        refreshRoutes()
        do { try selectRoute(selectedRoute) }
        catch { selectedRoute = .system; try? selectRoute(.system) }
        scheduleReconcile()
    }

    func audioSessionDidDeactivate(for id: UUID) {
        guard callID == id else { return }
        audioActivated = false
        do { try applyAudioGate() } catch { self.error = "Couldn't pause call audio." }
        refreshRoutes()
    }

    /// Called from CallKit's mute action, not directly by the mute button.
    func setMicrophoneEnabled(_ enabled: Bool, for id: UUID) async throws {
        try require(id)
        if enabled {
            let granted: Bool
            if AVCaptureDevice.authorizationStatus(for: .audio) == .authorized { granted = true }
            else if UIApplication.shared.applicationState == .active { granted = await LiveKitSDK.ensureDeviceAccess(for: [.audio]) }
            else { granted = false }
            try require(id)
            guard granted else { throw Failure.microphonePermission }
        }
        wantsMicrophone = enabled; revision += 1
        do {
            try applyAudioGate() // Close capture immediately, even if an older publish is in flight.
            try await reconcile()
        } catch {
            if callID == id, wantsMicrophone == enabled {
                wantsMicrophone = false; revision += 1
                try? applyAudioGate()
            }
            throw error
        }
    }

    func setCameraEnabled(_ enabled: Bool) async throws {
        guard let id = callID else { throw Failure.ended }
        if enabled {
            guard UIApplication.shared.applicationState == .active,
                  await LiveKitSDK.ensureDeviceAccess(for: [.video]) else { throw Failure.cameraPermission }
            try require(id)
        }
        wantsCamera = enabled; revision += 1
        try await reconcile()
    }

    func switchCamera() async throws {
        guard let id = callID, let connection = room,
              let track = connection.localParticipant.videoTracks.first(where: { $0.source == .camera })?.track as? LocalVideoTrack,
              let capturer = track.capturer as? CameraCapturer else { throw Failure.ended }
        _ = try await capturer.switchCameraPosition()
        try require(id); refreshSnapshot(connection)
    }

    func setOutputMuted(_ muted: Bool) async throws {
        guard callID != nil else { throw Failure.ended }
        outputMuted = muted; revision += 1
        try applyAudioGate() // Never wait for the network to silence output.
        try await reconcile()
    }

    func selectRoute(_ route: Route) throws {
        guard audioActivated else { throw Failure.waitingForAudio }
        let session = AVAudioSession.sharedInstance()
        switch route {
        case .speaker:
            try session.setPreferredInput(nil)
            try session.overrideOutputAudioPort(.speaker)
        case .system:
            try session.overrideOutputAudioPort(.none)
            try session.setPreferredInput(nil)
        case .input(let uid):
            guard let input = session.availableInputs?.first(where: { $0.uid == uid }) else { throw Failure.media }
            try session.overrideOutputAudioPort(.none)
            try session.setPreferredInput(input)
        }
        selectedRoute = route; refreshRoutes()
    }

    func updateNames(_ names: [String: String]) {
        self.names = names
        if let room { refreshSnapshot(room) }
    }

    /// Await before replacing the call, changing server, signing out, or discarding this store.
    func leave(for id: UUID) async {
        guard callID == id, phase != .ending else { return }
        pictureInPicture.end()
        phase = .ending; audioActivated = false
        wantsMicrophone = false; wantsCamera = false; outputMuted = true; revision += 1
        try? applyAudioGate()
        let pending = reconcileTask
        pending?.cancel()
        let connection = room; room = nil
        connection?.remove(delegate: self)
        if let routeObserver { NotificationCenter.default.removeObserver(routeObserver); self.routeObserver = nil }
        await connection?.disconnect()
        _ = try? await pending?.value
        reconcileTask = nil; subscriptions = [:]
        videos = []; people = []; inputs = []; otherDevices = 0
        microphoneEnabled = false; cameraEnabled = false; joinedQuietly = false
        channelID = nil; callID = nil; phase = .idle
        // Do not deactivate AVAudioSession. CallKit owns that transition.
    }

    func clearError() { error = nil }

    private func require(_ id: UUID) throws {
        guard callID == id, phase != .ending, phase != .failed else { throw Failure.ended }
        try Task.checkCancellation()
    }

    private func applyAudioGate() throws {
        try AudioManager.shared.setEngineAvailability(.init(
            isInputAvailable: audioActivated && wantsMicrophone,
            isOutputAvailable: audioActivated && !outputMuted))
    }

    private func reconcile() async throws {
        if let reconcileTask { return try await reconcileTask.value }
        guard let id = callID, let connection = room else { return }
        let task = Task { @MainActor [weak self] in
            guard let self else { return }
            repeat {
                try self.require(id)
                let targetRevision = self.revision
                for participant in connection.remoteParticipants.values {
                    guard let identity = participant.identity?.stringValue else { continue }
                    for case let publication as RemoteTrackPublication in participant.trackPublications.values {
                        try self.require(id)
                        let subscribe = CallPolicy.shouldSubscribe(isAudio: publication.kind == .audio,
                            isMicrophone: publication.source == .microphone, identity: identity,
                            accountID: self.accountID, outputMuted: self.outputMuted)
                        let key = identity + "/" + publication.sid.stringValue
                        if self.subscriptions[key] != subscribe {
                            try await publication.set(subscribed: subscribe)
                            try self.require(id); self.subscriptions[key] = subscribe
                        }
                    }
                }
                let mic = self.wantsMicrophone
                if connection.localParticipant.isMicrophoneEnabled() != mic {
                    _ = try await connection.localParticipant.setMicrophone(enabled: mic)
                    try self.require(id)
                }
                let camera = self.wantsCamera
                if connection.localParticipant.isCameraEnabled() != camera {
                    _ = try await connection.localParticipant.setCamera(enabled: camera)
                    try self.require(id)
                }
                self.refreshSnapshot(connection)
                if targetRevision == self.revision { return }
            } while true
        }
        reconcileTask = task
        defer { if self.callID == id { reconcileTask = nil } }
        try await task.value
    }

    private func scheduleReconcile() {
        guard let expected = callID else { return }
        revision += 1
        Task { @MainActor [weak self] in
            do { try await self?.reconcile() }
            catch { if self?.callID == expected, self?.isActive == true { self?.error = "Couldn't update call media. Try that control again." } }
        }
    }

    func receive(_ source: Room, event: CallRoomEvent) {
        guard room === source, let id = callID, phase != .ending else { return }
        switch event {
        case .presentation: refreshSnapshot(source)
        case .publications: refreshSnapshot(source); scheduleReconcile()
        case .reconnecting: phase = .reconnecting
        case .reconnected:
            phase = .connected; subscriptions = [:]; refreshSnapshot(source); scheduleReconcile()
        case .disconnected:
            pictureInPicture.end()
            phase = .failed; audioActivated = false; wantsMicrophone = false
            try? applyAudioGate()
            error = "The call disconnected."
            onUnexpectedDisconnect?(id)
        }
    }

    private func refreshSnapshot(_ connection: Room) {
        let all: [Participant] = [connection.localParticipant] + Array(connection.remoteParticipants.values)
        var groups: [String: [Participant]] = [:]
        var tiles: [CallVideo] = []
        for participant in all {
            guard let identity = participant.identity?.stringValue, let userID = CallPolicy.accountID(identity) else { continue }
            groups[userID, default: []].append(participant)
            let local = participant === connection.localParticipant
            let name = names[userID] ?? participant.name ?? "Someone"
            for publication in participant.videoTracks {
                tiles.append(CallVideo(id: identity + "/" + publication.sid.stringValue,
                    personID: userID, name: name, isLocal: local,
                    isShare: publication.source == .screenShareVideo, isMuted: publication.isMuted,
                    track: publication.track as? VideoTrack))
            }
        }
        people = groups.map { userID, members in
            CallPerson(id: userID, name: names[userID] ?? members.first?.name ?? "Someone",
                deviceCount: members.count, isYou: userID == accountID,
                isSpeaking: members.contains(where: \.isSpeaking))
        }.sorted { $0.name.localizedStandardCompare($1.name) == .orderedAscending }
        videos = tiles.sorted { $0.isShare != $1.isShare ? $0.isShare : $0.id < $1.id }
        otherDevices = max(0, (groups[accountID]?.count ?? 0) - 1)
        microphoneEnabled = connection.localParticipant.isMicrophoneEnabled()
        cameraEnabled = connection.localParticipant.isCameraEnabled()
        pictureInPicture.update(callID: isActive ? callID : nil, videos: videos)
    }

    private func observeRoutes() {
        routeObserver = NotificationCenter.default.addObserver(forName: AVAudioSession.routeChangeNotification,
            object: nil, queue: .main) { [weak self] _ in
                Task { @MainActor [weak self] in self?.refreshRoutes() }
            }
        refreshRoutes()
    }

    private func refreshRoutes() {
        let session = AVAudioSession.sharedInstance()
        inputs = (session.availableInputs ?? []).map { CallAudioInput(id: $0.uid, name: $0.portName) }
        if case .input(let uid) = selectedRoute, !inputs.contains(where: { $0.id == uid }) { selectedRoute = .system }
        routeLabel = session.currentRoute.outputs.map(\.portName).joined(separator: ", ")
        if routeLabel.isEmpty { routeLabel = "System audio" }
    }
}

struct CallPerson: Identifiable {
    let id: String
    let name: String
    let deviceCount: Int
    let isYou: Bool
    let isSpeaking: Bool
}

struct CallVideo: Identifiable {
    let id: String
    let personID: String
    let name: String
    let isLocal: Bool
    let isShare: Bool
    let isMuted: Bool
    let track: VideoTrack?
}

struct CallAudioInput: Identifiable {
    let id: String
    let name: String
}
