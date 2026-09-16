import Foundation

/// Which conversation a read state belongs to. A channel ID and a thread ID are
/// both ULIDs, so keeping them in one namespace would let an unlucky pair
/// collide; the enum makes that impossible by construction rather than by a
/// string prefix convention somebody has to remember.
enum ReadKey: Hashable, Sendable {
    case channel(String)
    case thread(String)
}

/// Read states arrive two ways: as the response to a read this client sent, and
/// as a WebSocket update the server pushed. Applying whichever lands last is
/// wrong, because a slow response carries a count that was already true when the
/// request was made and is stale by the time it returns.
///
/// Each key keeps a version. A pushed update or an authoritative refresh bumps
/// it; a response carries the version its request was issued at and is applied
/// only if nothing has superseded it since. Nothing here is thread-specific:
/// the activation PR reuses it by passing `.thread` keys.
@MainActor final class ReadOrdering {
    private var versions: [ReadKey: Int] = [:]

    /// Issue the token for a read this client is about to send.
    func begin(_ key: ReadKey) -> Int {
        let next = (versions[key] ?? 0) + 1
        versions[key] = next
        return next
    }

    /// A response may be applied only while its token is still the newest for
    /// its key. `token: nil` is a pushed or refetched state, which is always
    /// authoritative and supersedes any response still in flight.
    func accept(_ key: ReadKey, token: Int?) -> Bool {
        guard let token else {
            versions[key] = (versions[key] ?? 0) + 1
            return true
        }
        return versions[key] == token
    }

    /// An authoritative snapshot of everything. Every in-flight response is now
    /// older than what this replaced it with.
    func invalidateAll() {
        for key in versions.keys { versions[key] = (versions[key] ?? 0) + 1 }
    }

    /// Cleared synchronously with the account, so a response from the previous
    /// account can never be accepted against the next one.
    func reset() { versions.removeAll() }
}
