import DenAPI
import Foundation

/// Parent supplies the generated-client implementation after the M5b schema lands.
/// This file intentionally requires API.CallInvitationState; no substitute wire type.
@MainActor protocol CallControllerAPI: AnyObject {
    /// Minimal Keychain restore plus identity validation, not the full chat refresh.
    func restoreCallSession() async throws -> CallControllerCredentials
    func isCurrent(_ credentials: CallControllerCredentials) -> Bool
    /// Redeem an optional one-use state-only ticket without auth, then authenticated GET fallback.
    /// The origin comes from the selected trusted server, never from a push field.
    func invitation(id: String, fetchTicket: String?) async throws -> API.CallInvitationState
    func invitations() async throws -> [API.CallInvitationState]
    func invite(channelID: String) async throws -> API.CallInvitation
    func accept(_ invitation: API.CallInvitation, answerID: UUID) async throws -> API.CallInvitationState
    func decline(_ invitation: API.CallInvitation) async throws
    func cancel(_ invitation: API.CallInvitation) async throws
    func end(_ invitation: API.CallInvitation) async throws
}

/// Local dependency context. User is the generated schema, not a copied user model.
struct CallControllerCredentials {
    let generation: UUID
    let service: DenService
    let user: API.User
    let names: [String: String]
}
