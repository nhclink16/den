import DenAPI
import Foundation

/// Decode the generated push projection. Never log raw payloads, tickets, or the result.
enum VoIPPushEnvelope {
    static func decode(_ payload: [AnyHashable: Any]) -> API.IncomingVoipCall? {
        guard payload["type"] as? String == "call_invite",
              let data = try? JSONSerialization.data(withJSONObject: payload),
              let value = try? JSONDecoder().decode(API.IncomingVoipCall.self, from: data),
              [value.invitationId, value.channelId, value.fromUserId].allSatisfy({
                  !$0.isEmpty && $0.utf8.count <= 128 && !$0.contains(where: \.isWhitespace)
              }), !value.fetchTicket.isEmpty, value.expiresAt > 0,
              Double(value.expiresAt) <= Date().addingTimeInterval(90).timeIntervalSince1970 else { return nil }
        return value
    }
}

/// One installation identity is shared by alert and VoIP registration adapters.
enum VoIPPushInstallation {
    static func clientID(defaults: UserDefaults = .standard) -> UUID {
        let key = "den.push-installation-id"
        if let saved = defaults.string(forKey: key), let id = UUID(uuidString: saved) { return id }
        let id = UUID(); defaults.set(id.uuidString, forKey: key); return id
    }
}
