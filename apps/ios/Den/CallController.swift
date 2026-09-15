import AVFoundation
import CallKit
import DenAPI
import LiveKit
import Observation

@MainActor @Observable final class CallController: NSObject {
    let session: CallSession
    private(set) var currentCallID: UUID?
    private(set) var audioSessionInUse = false
    var pendingSystemReports: Set<UUID> = []
    var preventsDictation: Bool { currentCallID != nil || session.callID != nil || audioSessionInUse || !pendingSystemReports.isEmpty }
    var error: String?
    @ObservationIgnored var beforeAudioPreparation: (@MainActor () -> Void)?
    @ObservationIgnored let api: any CallControllerAPI
    @ObservationIgnored let provider: CXProvider
    @ObservationIgnored let system = CXCallController()
    @ObservationIgnored var contexts: [UUID: CallControllerContext] = [:]
    @ObservationIgnored var cleanupTasks: [UUID: Task<Void, Never>] = [:]
    @ObservationIgnored var receipts: [UUID: CallControllerReceipt] = [:]
    @ObservationIgnored var terminalInvitations: [String: Date] = [:]
    @ObservationIgnored var audioLease = CallControllerAudioLease()
    /// Latest known display names. Credentials carry their own snapshot, taken when identity
    /// was restored, which a profile change arriving during that restore would leave stale.
    @ObservationIgnored private(set) var names: [String: String] = [:]

    init(session: CallSession, api: any CallControllerAPI) {
        self.session = session; self.api = api
        // Register a configured audio session with CallKit. Configuring it for the first time
        // inside performStart can make the system end the call before audio activation.
        var preparationError: String?
        do {
            let audioManager = AudioManager.shared
            audioManager.audioSession.isAutomaticConfigurationEnabled = false
            try audioManager.setEngineAvailability(.none)
            try CallSession.prepareAudioSessionForCallKit()
        }
        catch { preparationError = "Couldn't prepare call audio. Try starting the call again." }
        let configuration = CXProviderConfiguration()
        configuration.supportedHandleTypes = [.generic]
        configuration.maximumCallsPerCallGroup = 1; configuration.maximumCallGroups = 1
        configuration.supportsVideo = true
        provider = CXProvider(configuration: configuration)
        super.init()
        self.error = preparationError
        provider.setDelegate(self, queue: .main)
        session.onUnexpectedDisconnect = { [weak self] id in
            guard let self, let context = contexts[id] else { return }
            Task { await finish(context, reason: .failed, notifyServer: true) }
        }
    }

    func requestOutgoing(channelID: String, title: String, inviteDM: Bool) async throws {
        guard currentCallID == nil, session.callID == nil, audioLease.activated == nil else { throw CallSession.Failure.alreadyCalling }
        beforeAudioPreparation?()
        // Dictation uses a recording category. Restore CallKit's category before the
        // system transaction, not for the first time inside its start callback.
        try CallSession.prepareAudioSessionForCallKit()
        let context = CallControllerContext(id: UUID(), channelID: channelID, title: title,
            mode: inviteDM ? .outgoingDM : .hangout)
        contexts[context.id] = context; currentCallID = context.id
        let action = CXStartCallAction(call: context.id, handle: CXHandle(type: .generic, value: channelID))
        action.isVideo = false
        refreshProviderConfiguration()
        do { try await system.request(CXTransaction(action: action)) }
        catch { await finish(context, reason: .failed, notifyServer: false); throw error }
    }

    /// Display names change mid-call. `title` resolves a context's current name from the
    /// caller's user id for a reported incoming call, or from its channel otherwise.
    func updateNames(_ names: [String: String], title: @MainActor (String, String?) -> String?) {
        self.names = names
        session.updateNames(names)
        for context in contexts.values where isLive(context) {
            let from = context.mode == .incoming ? context.invitation?.fromUserId : nil
            guard let updated = title(context.channelID, from), !updated.isEmpty, updated != context.title else { continue }
            context.title = updated
            if session.callID == context.id { session.retitle(updated) }
            // Only an incoming call was ever reported with a caller name to correct.
            if let from { provider.reportCall(with: context.id, updated: callUpdate(name: updated, handle: from)) }
        }
    }

    func requestMute(_ isMuted: Bool) async throws {
        guard let id = currentCallID else { throw CallSession.Failure.ended }
        try await system.request(CXTransaction(action: CXSetMutedCallAction(call: id, muted: isMuted)))
    }

    func requestEnd() async throws {
        guard let id = currentCallID else { return }
        try await system.request(CXTransaction(action: CXEndCallAction(call: id)))
    }

    /// Synchronous revocation path: close audio and invalidate every async call continuation
    /// before AppStore closes its service. The matching CallKit deactivation retains its owner.
    func sessionInvalidated() {
        session.pictureInPicture.end()
        for context in Array(contexts.values) {
            guard !context.ended else { continue }
            context.ending = true
            context.work?.cancel(); context.prefetch?.cancel(); context.expiry?.cancel()
            session.audioSessionDidDeactivate(for: context.id)
            audioLease.end(context.id)
            for receipt in receipts.values where receipt.callID == context.id { receipt.finish(false) }
            Task { await finish(context, reason: .remoteEnded, notifyServer: false) }
        }
    }

    /// Parent calls before logging out, replacing the server, or closing its DenService.
    func stopForSessionChange() async {
        for context in Array(contexts.values) { await finish(context, reason: .remoteEnded, notifyServer: true) }
        for task in Array(cleanupTasks.values) { await task.value }
    }

    func start(_ action: CXStartCallAction) {
        let receipt = track(action)
        guard let context = contexts[action.callUUID], isLive(context), context.mode != .incoming else { receipt.finish(false); return }
        provider.reportOutgoingCall(with: context.id, startedConnectingAt: Date())
        context.work = Task { @MainActor [weak self] in
            guard let self else { return }
            do {
                let credentials = try await api.restoreCallSession()
                try check(context); guard api.isCurrent(credentials) else { throw CallSession.Failure.ended }
                context.credentials = credentials
                // The server only permits a caller already present in the LiveKit room to invite.
                try await connectMedia(context, credentials: credentials)
                if context.mode == .outgoingDM {
                    let invitation = try await api.invite(channelID: context.channelID)
                    // Retain the POST result before any GET or cancellation check can fail.
                    context.invitationID = invitation.id
                    context.invitation = invitation
                    guard isLive(context) else { await cleanupLate(invitation, context: context); return }
                    armExpiry(context, deadline: Date(timeIntervalSince1970: TimeInterval(invitation.expiresAt)))
                    let state = try await api.invitation(id: invitation.id, fetchTicket: nil)
                    try check(context)
                    receive(state)
                }
                try check(context)
                try prepareActivation(context)
                receipt.finish(true)
                reportConnectedIfReady(context)
            } catch {
                receipt.finish(false)
                if isLive(context) { await finish(context, reason: .failed, notifyServer: true) }
            }
        }
    }

    func answer(_ action: CXAnswerCallAction) {
        let receipt = track(action)
        guard let context = contexts[action.callUUID], isLive(context), context.mode == .incoming,
              !context.answering else { receipt.finish(false); return }
        beforeAudioPreparation?()
        context.answering = true
        context.prefetch?.cancel(); context.prefetch = nil
        context.work = Task { @MainActor [weak self] in
            guard let self else { return }
            do {
                // CallKit reporting has already happened, even after a cold launch.
                let credentials = try await api.restoreCallSession()
                try check(context); guard api.isCurrent(credentials) else { throw CallSession.Failure.ended }
                context.credentials = credentials
                guard let invitationID = context.invitationID else { throw CallSession.Failure.ended }
                let current = try await api.invitation(id: invitationID, fetchTicket: nil)
                try check(context)
                receive(current); try check(context)
                let accepted = try await api.accept(current.invitation, answerID: context.answerID)
                guard isLive(context) else { await cleanupLate(accepted, context: context); return }
                receive(accepted); try check(context)
                guard accepted.accepted.contains(where: {
                    $0.userId == credentials.user.id && UUID(uuidString: $0.answerId) == context.answerID
                }) else { throw CallSession.Failure.ended }
                context.expiry?.cancel()
                try await connectMedia(context, credentials: credentials)
                try check(context)
                try prepareActivation(context)
                context.answering = false
                receipt.finish(true)
            } catch {
                receipt.finish(false)
                if isLive(context) {
                    // A 409 can mean another device answered while this one was accepting.
                    if let id = context.invitationID, let state = try? await api.invitation(id: id, fetchTicket: nil) { receive(state) }
                    if isLive(context) { await finish(context, reason: .failed, notifyServer: true) }
                }
            }
        }
    }

    func end(_ action: CXEndCallAction) {
        let receipt = track(action)
        guard let context = contexts[action.callUUID] else { receipt.finish(true); return }
        context.ending = true; context.work?.cancel()
        Task { @MainActor in
            await finish(context, reason: nil, notifyServer: true, preserving: action.uuid)
            receipt.finish(true)
        }
    }

    func mute(_ action: CXSetMutedCallAction) {
        let receipt = track(action)
        guard let context = contexts[action.callUUID], isLive(context), context.mediaReady else { receipt.finish(false); return }
        Task { @MainActor in
            do {
                try await session.setMicrophoneEnabled(!action.isMuted, for: context.id)
                try check(context); receipt.finish(true)
            } catch { receipt.finish(false) }
        }
    }

    func timedOut(_ action: CXAction) {
        receipts[action.uuid]?.gate.timedOut()
        guard let call = action as? CXCallAction, let context = contexts[call.callUUID] else { return }
        context.ending = true; context.work?.cancel(); context.prefetch?.cancel()
        Task { await finish(context, reason: .failed, notifyServer: true) }
    }

    func reset() {
        audioLease.reset()
        audioSessionInUse = false
        pendingSystemReports.removeAll()
        for receipt in receipts.values { receipt.gate.timedOut() }
        for context in Array(contexts.values) {
            context.ending = true; context.work?.cancel(); context.prefetch?.cancel()
            Task { await finish(context, reason: nil, notifyServer: true) }
        }
    }

    func activated() {
        defer { audioSessionInUse = audioLease.activated != nil }
        let live = Set(system.callObserver.calls.filter { !$0.hasEnded }.map(\.uuid))
        guard let id = audioLease.activate(liveCallIDs: live),
              let context = contexts[id], isLive(context), context.mediaReady else { return }
        do { try session.audioSessionDidActivate(for: id) }
        catch { Task { await finish(context, reason: .failed, notifyServer: true) } }
    }

    func deactivated() {
        defer { audioSessionInUse = audioLease.activated != nil }
        guard let owner = audioLease.deactivate() else { return }
        session.audioSessionDidDeactivate(for: owner)
    }

    func connectMedia(_ context: CallControllerContext, credentials: CallControllerCredentials) async throws {
        try check(context); guard api.isCurrent(credentials) else { throw CallSession.Failure.ended }
        try await session.join(callID: context.id, channelID: context.channelID, title: context.title,
            accountID: credentials.user.id, service: credentials.service, names: joinNames(credentials))
        try check(context); guard api.isCurrent(credentials) else { throw CallSession.Failure.ended }
        context.mediaReady = true
        context.lastKnownOtherDevices = session.otherDevices
    }

    /// `join` assigns the names it is handed before its first suspension, so a rename that
    /// arrived while these credentials were being restored would be overwritten by their older
    /// snapshot. Resolve at join time instead. The credential snapshot still covers a call that
    /// starts before this controller has ever been told any names.
    func joinNames(_ credentials: CallControllerCredentials) -> [String: String] {
        names.isEmpty ? credentials.names : names
    }

    func prepareActivation(_ context: CallControllerContext) throws {
        try check(context)
        try session.configureAudioSession(for: context.id)
        guard audioLease.arm(context.id) else { throw CallSession.Failure.waitingForAudio }
    }

    func reportConnectedIfReady(_ context: CallControllerContext) {
        guard context.mode != .incoming, context.mediaReady, !context.reportedConnected,
              context.mode == .hangout || context.call?.state == .active else { return }
        context.reportedConnected = true
        provider.reportOutgoingCall(with: context.id, connectedAt: Date())
    }

    func finish(_ context: CallControllerContext, reason: CXCallEndedReason?, notifyServer: Bool, preserving actionID: UUID? = nil) async {
        guard contexts[context.id] === context, !context.ended else { return }
        context.ended = true; context.expiry?.cancel(); context.work?.cancel(); context.prefetch?.cancel()
        if session.callID == context.id, session.isConnected {
            context.lastKnownOtherDevices = session.otherDevices
        }
        let knownOtherDevices = context.lastKnownOtherDevices
        audioLease.end(context.id)
        for receipt in receipts.values where receipt.callID == context.id && receipt.action.uuid != actionID { receipt.finish(false) }
        if let reason { provider.reportCall(with: context.id, endedAt: Date(), reason: reason) }
        await session.leave(for: context.id)
        contexts.removeValue(forKey: context.id)
        receipts = receipts.filter { $0.value.callID != context.id }
        if currentCallID == context.id { currentCallID = nil }
        if let invitationID = context.invitationID { terminalInvitations[invitationID] = Date().addingTimeInterval(600) }
        if notifyServer { _ = scheduleCleanup(context, knownOtherDevices: knownOtherDevices) }
    }

    func scheduleCleanup(_ context: CallControllerContext, knownOtherDevices: Int?) -> Task<Void, Never> {
        if let existing = cleanupTasks[context.id] { return existing }
        // Start a fresh unstructured task: an ended CallKit action's work is already cancelled.
        let task = Task { @MainActor [weak self] in
            guard let self else { return }
            defer { cleanupTasks.removeValue(forKey: context.id) }
            await cleanup(context, knownOtherDevices: knownOtherDevices)
        }
        cleanupTasks[context.id] = task
        return task
    }

    func adoptIncoming(_ id: UUID) {
        beforeAudioPreparation?()
        currentCallID = id
        do { try CallSession.prepareAudioSessionForCallKit() }
        catch { self.error = "Couldn't prepare call audio. Try answering the call again." }
    }

    func isLive(_ context: CallControllerContext) -> Bool { contexts[context.id] === context && !context.ended && !context.ending }
    func check(_ context: CallControllerContext) throws {
        guard isLive(context) else { throw CallSession.Failure.ended }
        try Task.checkCancellation()
    }
    func track(_ action: CXAction) -> CallControllerReceipt {
        let receipt = CallControllerReceipt(action)
        receipts[action.uuid] = receipt
        return receipt
    }
    func refreshProviderConfiguration() { provider.configuration = provider.configuration }
}

@MainActor final class CallControllerContext {
    enum Mode { case incoming, outgoingDM, hangout }
    let id: UUID
    let channelID: String
    let mode: Mode
    let answerID = UUID()
    var title: String
    var invitationID: String?
    var invitation: API.CallInvitation?
    var call: API.CallInvitationState?
    var signalRevision = 0
    var credentials: CallControllerCredentials?
    var mediaReady = false
    var lastKnownOtherDevices: Int?
    var answering = false
    var reportedConnected = false
    var ending = false
    var ended = false
    var work: Task<Void, Never>?
    var prefetch: Task<Void, Never>?
    var expiry: Task<Void, Never>?
    init(id: UUID, channelID: String, title: String, mode: Mode) {
        self.id = id; self.channelID = channelID; self.title = title; self.mode = mode
    }
}

@MainActor final class CallControllerReceipt {
    let action: CXAction
    let callID: UUID?
    var gate = CallControllerActionGate()
    init(_ action: CXAction) { self.action = action; callID = (action as? CXCallAction)?.callUUID }
    func finish(_ success: Bool) {
        guard gate.claimCompletion(now: Date(), deadline: action.timeoutDate, alreadyComplete: action.isComplete) else { return }
        if success { action.fulfill() } else { action.fail() }
    }
}
