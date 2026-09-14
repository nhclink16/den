import Foundation

/// Pure presentation/subscription rules. These are not transport/API models.
enum CallPolicy {
    static func accountID(_ identity: String) -> String? {
        guard let first = identity.split(separator: ":", maxSplits: 1, omittingEmptySubsequences: false).first,
              !first.isEmpty else { return nil }
        return String(first)
    }

    static func joinsQuietly(accountID: String, localIdentity: String, remoteIdentities: [String]) -> Bool {
        remoteIdentities.contains { $0 != localIdentity && Self.accountID($0) == accountID }
    }

    static func shouldSubscribe(isAudio: Bool, isMicrophone: Bool, identity: String,
                                accountID: String, outputMuted: Bool) -> Bool {
        !isAudio || (!outputMuted && !(isMicrophone && Self.accountID(identity) == accountID))
    }
}
