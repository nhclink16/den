import Foundation
import Observation
import PushKit

/// Adapter uses generated RegisterDevice/Device with purpose .voip and explicit APNs environment.
/// Alert registration uses the same installation ID, but a separate receipt/token key.
@MainActor protocol VoIPPushRegistration: AnyObject {
    func registerVoIP(token: Data, clientID: UUID) async throws
    func removeVoIPRegistration() async throws
}

@MainActor protocol VoIPCallReporting: AnyObject {
    func incoming(_ envelope: API.IncomingVoipCall?, mustReport: Bool, completion: @escaping @Sendable () -> Void)
}

extension CallController: VoIPCallReporting {}

@MainActor @Observable final class VoIPPushController: NSObject {
    var status: String?
    @ObservationIgnored let calls: any VoIPCallReporting
    @ObservationIgnored let registration: any VoIPPushRegistration
    @ObservationIgnored private var registry: PKPushRegistry?
    @ObservationIgnored private var token: Data?
    @ObservationIgnored private var generation = UUID()
    @ObservationIgnored private var acceptingRegistration = false
    @ObservationIgnored private var logoutSuspended = false
    @ObservationIgnored private var registrationTask: Task<Void, Never>?

    init(calls: any VoIPCallReporting, registration: any VoIPPushRegistration) {
        self.calls = calls; self.registration = registration
        super.init()
    }

    /// Invoke on every launch, including a PushKit background launch, before restoring chat.
    func start() {
        guard registry == nil else { return }
        let value = PKPushRegistry(queue: .main)
        value.delegate = self
        registry = value
        value.desiredPushTypes = [.voIP]
    }

    func sessionRestored() async {
        // Foreground/socket restoration is not a new login. Keep registration suspended
        // while the parent finishes alert-token deletion and server logout too.
        guard !logoutSuspended else { return }
        acceptingRegistration = true
        if let token { await scheduleRegistration(token).value }
    }

    func unregisterForLogout() async throws {
        let wasAccepting = acceptingRegistration
        let wasSuspended = logoutSuspended
        logoutSuspended = true
        acceptingRegistration = false; generation = UUID()
        let expected = generation
        let task = enqueue { [self] in
            guard generation == expected else { throw CancellationError() }
            try await registration.removeVoIPRegistration()
        }
        do {
            try await task.value
            guard generation == expected else { throw CancellationError() }
            token = nil
            registry?.desiredPushTypes = []
            // Parent removes alert registration too, before deleting the stored session.
        } catch {
            if generation == expected {
                logoutSuspended = wasSuspended
                acceptingRegistration = wasAccepting
                registry?.desiredPushTypes = [.voIP]
                if let token { await scheduleRegistration(token).value }
            }
            throw error
        }
    }

    func resumeAfterLogin() async {
        logoutSuspended = false
        generation = UUID(); acceptingRegistration = true
        registry?.desiredPushTypes = [.voIP]
        if let current = registry?.pushToken(for: .voIP) { token = current }
        if let token { await scheduleRegistration(token).value }
    }

    /// Every registration, invalidation, and logout delete enters the same ordered queue.
    /// A delayed invalidation can never delete a receipt created by a later token update.
    private func enqueue<Value: Sendable>(_ operation: @escaping @MainActor () async throws -> Value) -> Task<Value, Error> {
        let previous = registrationTask
        let task = Task { @MainActor in
            await previous?.value
            return try await operation()
        }
        registrationTask = Task { _ = try? await task.value }
        return task
    }

    private func scheduleRegistration(_ data: Data) -> Task<Void, Never> {
        let expected = generation
        let task = enqueue { [self] in
            guard acceptingRegistration, generation == expected, token == data else { return }
            do {
                try await registration.registerVoIP(token: data, clientID: VoIPPushInstallation.clientID())
                if generation == expected { status = nil }
            } catch {
                if generation == expected { status = "Incoming call notifications are not registered yet." }
            }
        }
        return Task { _ = try? await task.value }
    }

    func tokenChanged(_ data: Data) {
        token = data
        _ = scheduleRegistration(data)
    }

    func tokenInvalidated() {
        token = nil; generation = UUID()
        let expected = generation
        _ = enqueue { [self] in
            guard generation == expected else { return }
            do { try await registration.removeVoIPRegistration() }
            catch { if generation == expected { status = "An old call-notification registration could not be removed." } }
        }
    }

}

extension VoIPPushController: PKPushRegistryDelegate {
    nonisolated func pushRegistry(_ registry: PKPushRegistry, didUpdate pushCredentials: PKPushCredentials, for type: PKPushType) {
        guard type == .voIP else { return }
        let data = pushCredentials.token
        MainActor.assumeIsolated { tokenChanged(data) }
    }
    nonisolated func pushRegistry(_ registry: PKPushRegistry, didInvalidatePushTokenFor type: PKPushType) {
        guard type == .voIP else { return }
        MainActor.assumeIsolated { tokenInvalidated() }
    }
    nonisolated func pushRegistry(_ registry: PKPushRegistry, didReceiveIncomingPushWith payload: PKPushPayload,
                                 for type: PKPushType, completion: @escaping @Sendable () -> Void) {
        guard type == .voIP else { completion(); return }
        let envelope = VoIPPushEnvelope.decode(payload.dictionaryPayload)
        MainActor.assumeIsolated { calls.incoming(envelope, mustReport: true, completion: completion) }
    }
    @available(iOS 26.4, *)
    nonisolated func pushRegistry(_ registry: PKPushRegistry, didReceiveIncomingVoIPPushWith payload: PKPushPayload,
                                 metadata: PKVoIPPushMetadata, withCompletionHandler completion: @escaping @Sendable () -> Void) {
        let envelope = VoIPPushEnvelope.decode(payload.dictionaryPayload)
        let mustReport = metadata.mustReport
        MainActor.assumeIsolated { calls.incoming(envelope, mustReport: mustReport, completion: completion) }
    }
}
