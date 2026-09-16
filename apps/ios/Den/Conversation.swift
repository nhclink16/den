import Foundation

/// Which conversation a draft, an attachment queue or a read position belongs to.
///
/// A thread's identity is its channel plus its ROOT message, not the server's
/// thread ID: replying opens a conversation before any thread exists, and the ID
/// the server assigns on the first reply must not move the draft or its files to
/// a different owner. The room is the channel itself.
///
/// Nothing in this PR constructs `.thread`; the flat UI only ever uses `.room`.
/// It exists now so the activation PR inherits ownership rather than retrofitting
/// it under a panel that can be pushed and popped.
enum Conversation: Hashable, Sendable {
    case room(String)
    case thread(channelId: String, rootId: String)

    /// Uploads are begun against a channel on the server no matter which
    /// conversation they were staged in; only the client queue is per conversation.
    var channelId: String {
        switch self {
        case let .room(id): return id
        case let .thread(channelId, _): return channelId
        }
    }
}

/// What a composer had when it submitted. A completion may clear the box only if
/// all three still match: a different conversation, a changed quote target, or any
/// edit since submitting means what is on screen is no longer what was sent.
///
/// `revision` counts edits rather than comparing text, so typing A -> B -> A during
/// a send is a new draft and survives. It lives in the store, so navigating away
/// and back cannot reset it by destroying the view that held it.
struct DraftIdentity: Equatable, Sendable {
    let conversation: Conversation
    let replyToId: String?
    let revision: Int
}

struct Draft: Sendable {
    var text: String = ""
    var replyToId: String?
    var revision: Int = 0
}
