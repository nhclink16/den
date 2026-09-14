import DenAPI
import Foundation

/// Generated API transport shared by CallKit and PushKit. Push data never chooses an origin.
@MainActor final class NativeCallAPI: CallControllerAPI, VoIPPushRegistration {
    weak var store: AppStore?
    private var registeredPush: (generation: UUID, token: Data)?
    private let anonymousService: @MainActor (URL) -> DenService

    init(store: AppStore, anonymousService: @escaping @MainActor (URL) -> DenService = { DenService(origin: $0) }) {
        self.store = store; self.anonymousService = anonymousService
    }

    func restoreCallSession() async throws -> CallControllerCredentials {
        guard let store else { throw DenFailure.signedOut }
        let expected = store.generation
        if store.service == nil {
            guard let token = try SessionVault.read(store.origin) else { throw DenFailure.signedOut }
            store.service = DenService(origin: store.origin, token: token)
        }
        let service = try store.activeService()
        let identity = try await service.me()
        try store.check(expected)
        guard store.service === service else { throw DenFailure.signedOut }
        if let current = store.user, current.id != identity.id { throw DenFailure.signedOut }
        store.user = identity
        return .init(generation: expected, service: service, user: identity,
                     names: Dictionary(uniqueKeysWithValues: store.users.map { ($0.id, $0.displayName) }))
    }

    func isCurrent(_ credentials: CallControllerCredentials) -> Bool {
        guard let store else { return false }
        return store.generation == credentials.generation && store.service === credentials.service
            && store.user?.id == credentials.user.id
    }

    func invitation(id: String, fetchTicket: String?) async throws -> API.CallInvitationState {
        guard let store else { throw DenFailure.signedOut }
        let expected = store.generation
        if let fetchTicket {
            // The ticket is already a narrow capability. Do not attach the account bearer.
            let unauthenticated = anonymousService(store.origin)
            defer { unauthenticated.close() }
            var redeemed: API.CallInvitationState?
            do {
                redeemed = try await unauthenticated.client.postCallsInvitationsRedeem(body: .json(.init(ticket: fetchTicket))).ok.body.json
                try store.check(expected)
            } catch {
                try store.check(expected)
                if DenFailure.cancelled(error) || Task.isCancelled { throw CancellationError() }
                redeemed = nil
                // Duplicate pushes or expired tickets reconcile through the authenticated endpoint.
            }
            if let redeemed {
                guard redeemed.invitation.id == id else { throw DenFailure.invalidResponse }
                return redeemed
            }
        }
        let credentials = try await restoreCallSession()
        let call = try await credentials.service.client.getCallsInvitationsInvitationId(path: .init(invitationId: id)).ok.body.json
        guard isCurrent(credentials), call.invitation.id == id else { throw DenFailure.signedOut }
        return call
    }

    func invitations() async throws -> [API.CallInvitationState] {
        let credentials = try await restoreCallSession()
        let states = try await credentials.service.client.getCallsInvitations().ok.body.json
        guard isCurrent(credentials) else { throw DenFailure.signedOut }
        return states
    }

    func invite(channelID: String) async throws -> API.CallInvitation {
        let (store, service, expected) = try context()
        let invitation = try await service.client.postCallsChannelIdInvite(path: .init(channelId: channelID)).ok.body.json
        try store.check(expected)
        return invitation
    }

    func accept(_ invitation: API.CallInvitation, answerID: UUID) async throws -> API.CallInvitationState {
        let (store, service, expected) = try context()
        let result = try await service.client.postCallsChannelIdInviteAccept(path: .init(channelId: invitation.channelId),
            body: .json(.init(answerId: answerID.uuidString, invitationId: invitation.id))).ok.body.json
        try store.check(expected)
        return result
    }

    func decline(_ invitation: API.CallInvitation) async throws {
        let (store, service, expected) = try context()
        _ = try await service.client.postCallsChannelIdInviteDecline(path: .init(channelId: invitation.channelId),
            body: .json(.init(expiresAt: invitation.expiresAt, fromUserId: invitation.fromUserId, invitationId: invitation.id))).noContent
        try store.check(expected)
    }

    func cancel(_ invitation: API.CallInvitation) async throws {
        let (store, service, expected) = try context()
        _ = try await service.client.postCallsChannelIdInviteCancel(path: .init(channelId: invitation.channelId),
            body: .json(.init(invitationId: invitation.id))).noContent
        try store.check(expected)
    }

    func end(_ invitation: API.CallInvitation) async throws {
        let (store, service, expected) = try context()
        _ = try await service.client.postCallsChannelIdInviteEnd(path: .init(channelId: invitation.channelId),
            body: .json(.init(invitationId: invitation.id))).noContent
        try store.check(expected)
    }

    func registerVoIP(token: Data, clientID: UUID) async throws {
        let (store, service, expected) = try context()
        if let registeredPush, registeredPush.generation == expected, registeredPush.token == token { return }
        let key = pushKey(store)
        let version = Bundle.main.object(forInfoDictionaryKey: "CFBundleShortVersionString") as? String ?? "0"
        let environment = Bundle.main.object(forInfoDictionaryKey: "DenAPNSEnvironment") as? String ?? "sandbox"
        let device = try await service.client.postDevices(body: .json(.init(appVersion: version,
            clientId: clientID.uuidString, environment: environment, platform: .ios, purpose: .voip,
            token: token.map { String(format: "%02x", $0) }.joined()))).ok.body.json
        try store.check(expected)
        UserDefaults.standard.set(device.id, forKey: key)
        registeredPush = (expected, token)
    }

    func removeVoIPRegistration() async throws {
        guard let store else { return }
        let key = pushKey(store)
        guard let id = UserDefaults.standard.string(forKey: key) else { registeredPush = nil; return }
        do { _ = try await store.activeService().client.deleteDevicesId(path: .init(id: id)).noContent }
        catch {
            let underlying = (error as? ClientError)?.underlyingError ?? error
            if case DenFailure.server(404, _) = underlying { }
            else if !DenFailure.unauthorized(error) { throw error }
        }
        UserDefaults.standard.removeObject(forKey: key)
        registeredPush = nil
    }

    private func pushKey(_ store: AppStore) -> String { "den.voip-device:\(store.origin.absoluteString):\(store.user?.id ?? "")" }
    private func context() throws -> (AppStore, DenService, UUID) {
        guard let store, store.user != nil else { throw DenFailure.signedOut }
        return (store, try store.activeService(), store.generation)
    }
}
