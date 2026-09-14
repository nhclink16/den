import CryptoKit
import Foundation

/// Local CallKit identities and race guards, not API response models.
enum CallControllerPolicy {
    static func systemUUID(invitationID: String) -> UUID {
        if let uuid = UUID(uuidString: invitationID) { return uuid }
        let digest = Array(SHA256.hash(data: Data(("app.denchat.ios.invitation:" + invitationID).utf8)))
        return UUID(uuid: (digest[0], digest[1], digest[2], digest[3], digest[4], digest[5],
            (digest[6] & 0x0f) | 0x80, digest[7], (digest[8] & 0x3f) | 0x80, digest[9],
            digest[10], digest[11], digest[12], digest[13], digest[14], digest[15]))
    }

    static func ownsAnswer(accountID: String?, answerID: UUID, acceptances: [(String, String)]) -> Bool {
        guard let accountID else { return false }
        return acceptances.contains { $0.0 == accountID && UUID(uuidString: $0.1) == answerID }
    }

    /// /end removes invitation signaling; it is not a command to disconnect other peers.
    static func endedInvitationClosesLocalCall(hasConnectedMedia: Bool, hasAcceptedOwnAnswer: Bool) -> Bool {
        !hasConnectedMedia && !hasAcceptedOwnAnswer
    }

    static func expiresRinging(now: Date, deadline: Date, accepted: Bool, answering: Bool) -> Bool {
        now >= deadline && !accepted && !answering
    }
}

struct CallControllerActionGate {
    private(set) var completed = false
    mutating func claimCompletion(now: Date, deadline: Date, alreadyComplete: Bool) -> Bool {
        guard !completed else { return false }
        completed = true
        // CallKit inherently fails timed-out actions. Never settle those a second time.
        return !alreadyComplete && now < deadline
    }
    mutating func timedOut() { completed = true }
}

struct CallControllerAudioLease {
    private(set) var armed: UUID?
    private(set) var activated: UUID?
    mutating func arm(_ id: UUID) -> Bool {
        guard activated == nil || activated == id, armed == nil || armed == id else { return false }
        armed = id
        return true
    }
    mutating func activate(liveCallIDs: Set<UUID>) -> UUID? {
        guard let armed, liveCallIDs.contains(armed), activated == nil || activated == armed else { return nil }
        activated = armed
        return armed
    }
    mutating func end(_ id: UUID) {
        if armed == id { armed = nil }
        // Retain the activated owner until didDeactivate, even after the media room ends.
    }
    mutating func deactivate() -> UUID? {
        let owner = activated
        activated = nil
        return owner
    }
    mutating func reset() { armed = nil; activated = nil }
}
