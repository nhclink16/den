import Foundation
import Testing
#if canImport(Den)
@testable import Den
#else
@testable import CallPolicyHarness
#endif

struct CallControllerPolicyTests {
    @Test func invitationIdentityIsStableAndDoesNotAliasSimilarServerIDs() {
        let expected = UUID(uuidString: "27EFDECE-DAFB-8277-ABD5-44116EC990FB")!
        #expect(CallControllerPolicy.systemUUID(invitationID: "01JTESTINVITATION0000000001") == expected)
        #expect(CallControllerPolicy.systemUUID(invitationID: "01JTESTINVITATION0000000002") != expected)
        #expect(CallControllerPolicy.systemUUID(invitationID: expected.uuidString.lowercased()) == expected)
    }

    @Test func callActionsSettleOnceAndNeverAfterTheSystemAlreadyCompletedThem() {
        let now = Date(timeIntervalSince1970: 100)
        let deadline = now.addingTimeInterval(5)
        var settled = CallControllerActionGate()
        let firstSettlement = settled.claimCompletion(now: now, deadline: deadline, alreadyComplete: false)
        #expect(firstSettlement)
        let secondSettlementRejected = !settled.claimCompletion(now: now, deadline: deadline, alreadyComplete: false)
        #expect(secondSettlementRejected)
        var timedOut = CallControllerActionGate()
        timedOut.timedOut()
        let systemTimeoutRejected = !timedOut.claimCompletion(now: now, deadline: deadline, alreadyComplete: false)
        #expect(systemTimeoutRejected)
        var deadlineReached = CallControllerActionGate()
        let deadlineSettlementRejected = !deadlineReached.claimCompletion(now: deadline, deadline: deadline, alreadyComplete: false)
        #expect(deadlineSettlementRejected)
        var systemCompleted = CallControllerActionGate()
        let completedActionRejected = !systemCompleted.claimCompletion(now: now, deadline: deadline, alreadyComplete: true)
        #expect(completedActionRejected)
    }

    @Test func audioActivationStaysWithTheArmedObservedCallUntilItsOwnDeactivation() {
        let old = UUID(), next = UUID()
        var lease = CallControllerAudioLease()
        let oldCallArmed = lease.arm(old)
        #expect(oldCallArmed)
        let unobservedActivationRejected = lease.activate(liveCallIDs: [next]) == nil
        #expect(unobservedActivationRejected)
        let oldCallActivated = lease.activate(liveCallIDs: [old]) == old
        #expect(oldCallActivated)
        lease.end(old)
        let replacementBlocked = !lease.arm(next)
        #expect(replacementBlocked)
        let oldCallOwnsDeactivation = lease.deactivate() == old
        #expect(oldCallOwnsDeactivation)
        let nextCallArmed = lease.arm(next)
        #expect(nextCallArmed)
        let nextCallActivated = lease.activate(liveCallIDs: [old, next]) == next
        #expect(nextCallActivated)
        let nextCallOwnsInterruption = lease.deactivate() == next
        #expect(nextCallOwnsInterruption)
        // An interruption can reactivate the same still-live call.
        let nextCallReactivated = lease.activate(liveCallIDs: [next]) == next
        #expect(nextCallReactivated)
        lease.end(next)
        let endedCallCannotReactivate = lease.activate(liveCallIDs: [next]) == nil
        #expect(endedCallCannotReactivate)
        let nextCallOwnsFinalDeactivation = lease.deactivate() == next
        #expect(nextCallOwnsFinalDeactivation)
        lease.reset()
        let resetCannotReactivate = lease.activate(liveCallIDs: [next]) == nil
        #expect(resetCannotReactivate)
    }

    @Test func signalingEndDismissesPendingCallsWithoutDisconnectingAcceptedOrConnectedPeers() {
        #expect(CallControllerPolicy.endedInvitationClosesLocalCall(hasConnectedMedia: false, hasAcceptedOwnAnswer: false))
        #expect(!CallControllerPolicy.endedInvitationClosesLocalCall(hasConnectedMedia: true, hasAcceptedOwnAnswer: false))
        #expect(!CallControllerPolicy.endedInvitationClosesLocalCall(hasConnectedMedia: false, hasAcceptedOwnAnswer: true))
        #expect(!CallControllerPolicy.endedInvitationClosesLocalCall(hasConnectedMedia: true, hasAcceptedOwnAnswer: true))
    }

    @Test func anotherGroupRecipientsAnswerDoesNotCancelThisDevicesRingingDeadline() {
        let answer = UUID(), otherAnswer = UUID()
        let deadline = Date(timeIntervalSince1970: 145)
        let others = [("bob", answer.uuidString), ("ann", otherAnswer.uuidString)]
        let mine = others + [("ann", answer.uuidString.lowercased())]
        let notMine = CallControllerPolicy.ownsAnswer(accountID: "ann", answerID: answer, acceptances: others)
        let own = CallControllerPolicy.ownsAnswer(accountID: "ann", answerID: answer, acceptances: mine)
        #expect(!notMine)
        #expect(own)
        #expect(!CallControllerPolicy.ownsAnswer(accountID: nil, answerID: answer, acceptances: mine))
        #expect(CallControllerPolicy.expiresRinging(now: deadline, deadline: deadline, accepted: notMine, answering: false))
        #expect(!CallControllerPolicy.expiresRinging(now: deadline.addingTimeInterval(-0.01), deadline: deadline, accepted: false, answering: false))
        #expect(!CallControllerPolicy.expiresRinging(now: deadline.addingTimeInterval(60), deadline: deadline, accepted: own, answering: false))
        #expect(!CallControllerPolicy.expiresRinging(now: deadline, deadline: deadline, accepted: false, answering: true))
    }
}
