import Foundation

/// A draft and its attachment queue belong to one room or conversation.
/// A conversation uses the root ID because its server thread does not exist until
/// the first reply succeeds.
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

/// The native navigation target. Search and push may know only a thread ID until
/// metadata loads, while replying to a root starts with no thread ID at all.
struct ThreadSelection: Hashable, Sendable {
    let channelId: String
    var rootId: String?
    var threadId: String?
    var targetMessageId: String?
}

/// What a composer had when it submitted. A completion clears the box only when
/// the conversation, quote target and edit revision still match.
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
