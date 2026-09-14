import Foundation
import Testing
@testable import Den

@Suite(.serialized) struct VoIPPushControllerTests {
    @Test @MainActor func invalidationFinishesBeforeReplacementTokenCanCreateItsReceipt() async throws {
        let registration = ControlledVoIPRegistration()
        let controller = VoIPPushController(calls: UnusedVoIPCallReporter(), registration: registration)
        let old = Data([1]), replacement = Data([2])
        controller.tokenChanged(old)
        await controller.sessionRestored()
        #expect(registration.receipt == old)
        let originalRegistrations = registration.registeredTokens.count

        registration.pauseRemoval = true
        defer { registration.completeRemoval() }
        controller.tokenInvalidated()
        try await waitFor { registration.removalIsPending }
        controller.tokenChanged(replacement)
        let restored = Task { await controller.sessionRestored() }
        // Let a wrongly unsequenced registration execute while the delete is held open.
        try await Task.sleep(for: .milliseconds(30))
        #expect(registration.registeredTokens.count == originalRegistrations)
        registration.completeRemoval()
        await restored.value

        #expect(registration.receipt == replacement, "A delayed invalidation must not erase the new token's receipt.")
        let deleteFinished = try #require(registration.events.firstIndex(of: .removed))
        let replacementStarted = try #require(registration.events.firstIndex(of: .registered(replacement)))
        #expect(deleteFinished < replacementStarted)
    }

    @Test @MainActor func failedLogoutRetriesTheCachedTokenAndKeepsRegistrationEnabled() async throws {
        let registration = ControlledVoIPRegistration()
        let controller = VoIPPushController(calls: UnusedVoIPCallReporter(), registration: registration)
        let old = Data([3]), replacement = Data([4])
        controller.tokenChanged(old)
        await controller.sessionRestored()
        let originalRegistrations = registration.registeredTokens.count

        registration.pauseRemoval = true
        defer { registration.completeRemoval() }
        let logout = Task { try await controller.unregisterForLogout() }
        try await waitFor { registration.removalIsPending }
        registration.completeRemoval(throwing: ControlledVoIPRegistration.Failure.offline)
        do {
            try await logout.value
            Issue.record("Failed device deletion must make logout fail.")
        } catch ControlledVoIPRegistration.Failure.offline { }

        #expect(registration.registeredTokens.count == originalRegistrations + 1,
                "Interrupted logout must retry the cached token without waiting for a new OS callback.")
        #expect(registration.registeredTokens.last == old)
        #expect(registration.receipt == old)
        controller.tokenChanged(replacement)
        await controller.sessionRestored()
        #expect(registration.receipt == replacement)
        #expect(controller.status == nil)
    }

    @Test @MainActor func foregroundRestorationCannotRegisterAgainUntilAnExplicitLoginAfterLogout() async throws {
        let registration = ControlledVoIPRegistration()
        let controller = VoIPPushController(calls: UnusedVoIPCallReporter(), registration: registration)
        let old = Data([5]), replacement = Data([6])
        controller.tokenChanged(old)
        await controller.sessionRestored()
        let originalRegistrations = registration.registeredTokens.count

        registration.pauseRemoval = true
        defer { registration.completeRemoval() }
        let logout = Task { try await controller.unregisterForLogout() }
        try await waitFor { registration.removalIsPending }
        var restorationEntered = false
        let foregroundDuringDelete = Task {
            restorationEntered = true
            await controller.sessionRestored()
        }
        try await waitFor { restorationEntered }
        registration.completeRemoval()
        try await logout.value
        await foregroundDuringDelete.value

        // The parent's alert deletion/server logout may still be awaiting network responses.
        await controller.sessionRestored()
        controller.tokenChanged(replacement)
        await controller.sessionRestored()
        #expect(registration.registeredTokens.count == originalRegistrations,
                "Neither a foreground refresh nor a late token callback is an explicit login.")
        #expect(registration.receipt == nil)

        await controller.resumeAfterLogin()
        #expect(registration.registeredTokens.count == originalRegistrations + 1)
        #expect(registration.receipt == replacement)
    }

    @MainActor private func waitFor(_ condition: @MainActor () -> Bool) async throws {
        for _ in 0..<200 {
            if condition() { return }
            try await Task.sleep(for: .milliseconds(10))
        }
        try #require(condition(), "Controlled registration did not reach the expected suspension point.")
    }
}

@MainActor private final class ControlledVoIPRegistration: VoIPPushRegistration {
    enum Failure: Error { case offline }
    enum Event: Equatable { case registered(Data), removing, removed }
    var registeredTokens: [Data] = []
    var receipt: Data?
    var events: [Event] = []
    var pauseRemoval = false
    private var removal: CheckedContinuation<Void, Error>?
    var removalIsPending: Bool { removal != nil }

    func registerVoIP(token: Data, clientID: UUID) async throws {
        registeredTokens.append(token)
        events.append(.registered(token))
        receipt = token
    }

    func removeVoIPRegistration() async throws {
        events.append(.removing)
        if pauseRemoval {
            try await withCheckedThrowingContinuation { removal = $0 }
        }
        receipt = nil
        events.append(.removed)
    }

    func completeRemoval(throwing error: Error? = nil) {
        pauseRemoval = false
        let continuation = removal
        removal = nil
        if let error { continuation?.resume(throwing: error) }
        else { continuation?.resume() }
    }
}

@MainActor private final class UnusedVoIPCallReporter: VoIPCallReporting {
    func incoming(_ envelope: API.IncomingVoipCall?, mustReport: Bool, completion: @escaping @Sendable () -> Void) {
        Issue.record("Registration tests must not report a system call.")
        completion()
    }
}
