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
/// only if nothing has superseded it since. The typed key keeps channel and
/// thread read lifecycles independent.
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

    /// A filtered page cannot prove anything about keys it omitted. Capture the
    /// versions before loading it, then apply each returned key only if no live
    /// event or newer request changed that key while the page was in flight.
    func snapshot() -> [ReadKey: Int] { versions }

    func accept(_ key: ReadKey, from snapshot: [ReadKey: Int]) -> Bool {
        let captured = snapshot[key] ?? 0
        guard (versions[key] ?? 0) == captured else { return false }
        versions[key] = captured + 1
        return true
    }

    /// A channel snapshot supersedes only channel requests. Thread reads arrive
    /// from a separate endpoint and keep their own in-flight ordering.
    func invalidateChannels() {
        for key in versions.keys {
            guard case .channel = key else { continue }
            versions[key] = (versions[key] ?? 0) + 1
        }
    }

    /// Cleared synchronously with the account, so a response from the previous
    /// account can never be accepted against the next one.
    func reset() { versions.removeAll() }
}
