import CallKit
import DenAPI
import Foundation

extension CallController {
    /// Called only for the canonical call_invitation_state event. Ignore legacy duplicate invites.
    func receive(_ state: API.CallInvitationState) {
        guard let context = contexts.values.first(where: { $0.invitationID == state.invitation.id }),
              isLive(context) else { return }
        guard context.channelID == state.invitation.channelId else {
            endFromSignal(context, reason: .failed); return
        }
        let ownID = context.credentials?.user.id
        if context.mode == .incoming, let ownID {
            if let accepted = state.accepted.first(where: { $0.userId == ownID }),
               UUID(uuidString: accepted.answerId) != context.answerID {
                endFromSignal(context, reason: .answeredElsewhere); return
            }
            if state.declinedUserIds.contains(ownID) {
                endFromSignal(context, reason: .declinedElsewhere); return
            }
        }
        guard context.call != state else { return }
        context.call = state
        context.signalRevision += 1
        context.invitation = state.invitation
        switch state.state {
        case .ringing: break
        case .active:
            if context.mode != .incoming || hasWinningAnswer(context) { context.expiry?.cancel() }
            reportConnectedIfReady(context)
        case .expired:
            endFromSignal(context, reason: .unanswered)
        case .cancelled:
            endFromSignal(context, reason: .remoteEnded)
        case .ended:
            // The caller ended invitation signaling, not another participant's media connection.
            if CallControllerPolicy.endedInvitationClosesLocalCall(hasConnectedMedia: context.mediaReady,
                hasAcceptedOwnAnswer: hasWinningAnswer(context)) {
                endFromSignal(context, reason: .remoteEnded)
            }
        }
    }

    func endFromSignal(_ context: CallControllerContext, reason: CXCallEndedReason) {
        guard isLive(context) else { return }
        context.ending = true; context.work?.cancel(); context.prefetch?.cancel()
        Task { await finish(context, reason: reason, notifyServer: false) }
    }

    /// Parent invokes after reconnect/foreground. This does not create duplicate CallKit calls.
    func reconcileInvitations() async {
        do {
            for state in try await api.invitations() { await receiveAuthenticated(state) }
        } catch { self.error = "Couldn't refresh call status." }
    }

    /// Input must come from the authenticated, recipient-scoped canonical event or invitation list.
    /// Unlike PushKit delivery, this path may restore identity before deciding whether to report.
    func receiveAuthenticated(_ state: API.CallInvitationState) async {
        if contexts.values.contains(where: { $0.invitationID == state.invitation.id }) { receive(state); return }
        terminalInvitations = terminalInvitations.filter { $0.value > Date() }
        guard terminalInvitations[state.invitation.id] == nil,
              state.state == .ringing || state.state == .active,
              Date(timeIntervalSince1970: TimeInterval(state.invitation.expiresAt)) > Date() else { return }
        do {
            let credentials = try await api.restoreCallSession()
            guard api.isCurrent(credentials),
                  state.invitation.fromUserId != credentials.user.id,
                  !state.accepted.contains(where: { $0.userId == credentials.user.id }),
                  !state.declinedUserIds.contains(credentials.user.id) else { return }
            // Another event or a VoIP push may have reported while identity was restoring.
            if contexts.values.contains(where: { $0.invitationID == state.invitation.id }) { receive(state); return }
            guard currentCallID == nil, terminalInvitations[state.invitation.id] == nil else { return }
            let context = CallControllerContext(id: CallControllerPolicy.systemUUID(invitationID: state.invitation.id),
                channelID: state.invitation.channelId,
                title: credentials.names[state.invitation.fromUserId] ?? "Den call", mode: .incoming)
            context.invitationID = state.invitation.id; context.invitation = state.invitation
            context.credentials = credentials
            contexts[context.id] = context; adoptIncoming(context.id)
            refreshProviderConfiguration()
            provider.reportNewIncomingCall(with: context.id,
                update: callUpdate(name: context.title, handle: state.invitation.fromUserId)) { [weak self] reportError in
                Task { @MainActor [weak self] in
                    guard let self, isLive(context) else { return }
                    if reportError != nil { await finish(context, reason: nil, notifyServer: false); return }
                    armExpiry(context, deadline: Date(timeIntervalSince1970: TimeInterval(state.invitation.expiresAt)))
                    receive(context.call ?? state)
                }
            }
        } catch { self.error = "Couldn't restore this incoming call." }
    }

    func incoming(_ envelope: API.IncomingVoipCall?, mustReport: Bool, completion: @escaping @Sendable () -> Void) {
        terminalInvitations = terminalInvitations.filter { $0.value > Date() }
        guard let envelope else {
            guard mustReport else { completion(); return }
            reportMalformed(completion: completion)
            return
        }
        let existing = contexts.values.first { $0.invitationID == envelope.invitationId }
        if !mustReport, existing != nil || terminalInvitations[envelope.invitationId] != nil || Date(timeIntervalSince1970: TimeInterval(envelope.expiresAt)) <= Date() {
            completion(); return
        }
        let id = existing?.id ?? CallControllerPolicy.systemUUID(invitationID: envelope.invitationId)
        let context = existing ?? CallControllerContext(id: id, channelID: envelope.channelId, title: envelope.fromDisplayName, mode: .incoming)
        context.invitationID = envelope.invitationId
        let busy = currentCallID != nil && currentCallID != id
        if existing == nil {
            contexts[id] = context
            if !busy { adoptIncoming(id) }
        }
        let update = callUpdate(name: envelope.fromDisplayName, handle: envelope.fromUserId)
        refreshProviderConfiguration()
        // The report precedes every network request and every credential restore.
        provider.reportNewIncomingCall(with: id, update: update) { [weak self] reportError in
            completion()
            Task { @MainActor [weak self] in
                guard let self else { return }
                // A duplicate required push still reports the same UUID, then leaves its real call intact.
                if existing != nil { return }
                guard isLive(context) else { return }
                if reportError != nil { await finish(context, reason: nil, notifyServer: false); return }
                if busy { await finish(context, reason: .failed, notifyServer: false); return }
                if terminalInvitations[envelope.invitationId] != nil || Date(timeIntervalSince1970: TimeInterval(envelope.expiresAt)) <= Date() {
                    await finish(context, reason: .unanswered, notifyServer: false); return
                }
                armExpiry(context, deadline: Date(timeIntervalSince1970: TimeInterval(envelope.expiresAt)))
                context.prefetch = Task { @MainActor [weak self] in
                    guard let self else { return }
                    do {
                        let revision = context.signalRevision
                        let state = try await api.invitation(id: envelope.invitationId, fetchTicket: envelope.fetchTicket)
                        try check(context)
                        // Ticket redemption works before Keychain access. Restore is only after reporting.
                        if let credentials = try? await api.restoreCallSession(), api.isCurrent(credentials) { context.credentials = credentials }
                        try check(context)
                        // A canonical event received during this request is newer than its snapshot.
                        receive(context.signalRevision == revision ? state : (context.call ?? state))
                    } catch {
                        if !(error is CancellationError), isLive(context), !context.answering, !context.mediaReady {
                            await finish(context, reason: .failed, notifyServer: false)
                        }
                    }
                }
            }
        }
    }

    func reportMalformed(completion: @escaping @Sendable () -> Void) {
        let id = UUID()
        refreshProviderConfiguration()
        provider.reportNewIncomingCall(with: id, update: callUpdate(name: "Den call", handle: "Den")) { [weak self] reportError in
            completion()
            Task { @MainActor [weak self] in
                if reportError == nil { self?.provider.reportCall(with: id, endedAt: Date(), reason: .failed) }
            }
        }
    }

    func callUpdate(name: String, handle: String) -> CXCallUpdate {
        let update = CXCallUpdate()
        update.remoteHandle = CXHandle(type: .generic, value: handle)
        update.localizedCallerName = name
        update.hasVideo = false
        update.supportsDTMF = false; update.supportsHolding = false
        update.supportsGrouping = false; update.supportsUngrouping = false
        return update
    }

    func armExpiry(_ context: CallControllerContext, deadline: Date) {
        context.expiry?.cancel()
        context.expiry = Task { @MainActor [weak self] in
            do { try await Task.sleep(for: .seconds(max(0, deadline.timeIntervalSinceNow))) }
            catch { return }
            guard let self, isLive(context), CallControllerPolicy.expiresRinging(now: Date(), deadline: deadline,
                accepted: context.mode == .incoming ? hasWinningAnswer(context) : context.call?.state == .active,
                answering: context.answering) else { return }
            await finish(context, reason: .unanswered, notifyServer: true)
        }
    }

    func hasWinningAnswer(_ context: CallControllerContext) -> Bool {
        CallControllerPolicy.ownsAnswer(accountID: context.credentials?.user.id, answerID: context.answerID,
            acceptances: (context.call?.accepted ?? []).map { ($0.userId, $0.answerId) })
    }

    func cleanup(_ context: CallControllerContext, knownOtherDevices: Int?) async {
        guard context.mode != .hangout, let id = context.invitationID else { return }
        do {
            let credentials: CallControllerCredentials
            if let existing = context.credentials { credentials = existing }
            else { credentials = try await api.restoreCallSession() }
            guard api.isCurrent(credentials) else { return }
            let state: API.CallInvitationState
            do { state = try await api.invitation(id: id, fetchTicket: nil) }
            catch {
                // The POST result is retained even if the follow-up GET fails. Cancellation is
                // invitation-scoped and the server rejects a ringing-only cancel after acceptance.
                if context.mode == .outgoingDM, knownOtherDevices == 0,
                   let invitation = context.invitation, api.isCurrent(credentials) {
                    try await api.cancel(invitation)
                    return
                }
                throw error
            }
            guard api.isCurrent(credentials) else { return }
            if context.mode == .incoming {
                // Active means any group recipient accepted. This account may still be ringing.
                if state.state == .ringing || state.state == .active,
                   !state.accepted.contains(where: { $0.userId == credentials.user.id }),
                   !state.declinedUserIds.contains(credentials.user.id) {
                    try await api.decline(state.invitation)
                }
                // A recipient never calls the caller-only global end endpoint. Accepted-but-not-
                // joined answers are reclaimed by the server's authoritative abandonment check.
            } else if knownOtherDevices == 0 {
                if state.state == .ringing { try await api.cancel(state.invitation) }
                else if state.state == .active { try await api.end(state.invitation) }
            }
            // Unknown is not zero. Preserve another device's call without a room snapshot.
        } catch { self.error = "This device left, but the server call status could not be updated." }
    }

    func cleanupLate(_ invitation: API.CallInvitation, context: CallControllerContext) async {
        context.invitationID = invitation.id; context.invitation = invitation
        terminalInvitations[invitation.id] = Date().addingTimeInterval(600)
        await scheduleCleanup(context, knownOtherDevices: context.lastKnownOtherDevices).value
    }

    func cleanupLate(_ state: API.CallInvitationState, context: CallControllerContext) async {
        context.call = state
        await cleanupLate(state.invitation, context: context)
    }
}
