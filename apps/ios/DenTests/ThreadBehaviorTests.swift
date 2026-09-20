import DenAPI
import Foundation
import Synchronization
import Testing
import UserNotifications
@testable import Den

@Suite(.serialized) struct ThreadBehaviorTests {
    @Test @MainActor func realtimeKeepsRepliesOutOfTheRoomAndScopesTypingToTheirThread() async throws {
        let store = AppStore()
        let summary = thread()
        store.channels = [.init(id: "room", kind: .text, memberIds: [], name: "Room", position: 0)]
        store.messages["room"] = [message(id: "root")]
        store.threadMetadata[summary.id] = summary

        let reply = message(id: "reply-2", threadId: summary.id)
        var wire = try #require(try JSONSerialization.jsonObject(with: JSONEncoder().encode(reply)) as? [String: Any])
        wire["type"] = "message_created"
        try await store.receive(try JSONSerialization.data(withJSONObject: wire))
        try await store.receive(try JSONSerialization.data(withJSONObject: [
            "type": "typing", "channel_id": "room", "thread_id": summary.id, "user_id": "alice",
        ]))

        #expect(store.messages["room"]?.map(\.id) == ["root"])
        #expect(store.threadMessages[summary.id]?.map(\.id) == ["reply-2"])
        #expect(store.typing[.room("room")] == nil)
        #expect(store.typing[.thread(channelId: "room", rootId: "root")]?["alice"] != nil)
    }

    @Test @MainActor func pushedThreadReadWinsAResponseThatWasAlreadyInFlight() {
        let store = AppStore()
        let requestToken = store.readOrder.begin(.thread("thread-1"))
        let roomToken = store.readOrder.begin(.channel("room"))
        store.applyThreadRead(read(last: "reply-3", unread: 2))
        store.applyThreadRead(read(last: "reply-1", unread: 0), token: requestToken)
        store.applyChannelRead(.init(channelId: "room", lastReadId: "root", mentionCount: 0,
                                     notificationCount: 0, unreadCount: 0), token: roomToken)

        #expect(store.threadReadStates["thread-1"]?.lastReadId == "reply-3")
        #expect(store.threadReadStates["thread-1"]?.unreadCount == 2)
        #expect(store.readState("room")?.lastReadId == "root",
                "A thread push must not invalidate the independent room read request.")
    }

    @Test @MainActor func notificationTapAndForegroundSuppressionUseTheExactConversation() async {
        ThreadHTTPStub.reset()
        let store = serviceStore()
        defer { store.service?.close() }
        let controller = NotificationController(store: store,
            authorizationStatus: { .denied }, requestAuthorization: { false }, registerRemote: {})
        await controller.handleTap(channel: "room", message: "reply-2", thread: "thread-1")
        #expect(store.selectedChannelId == nil)
        store.user = .init(bot: false, displayName: "Me", id: "me", role: .member, username: "me")
        await controller.requestAfterLogin()

        #expect(store.selectedChannelId == "room")
        #expect(store.selectedThread == .init(channelId: "room", rootId: nil, threadId: "thread-1",
                                              targetMessageId: "reply-2"))
        #expect(await controller.presentationOptions(channel: "room", thread: "thread-1") == [.badge])
        #expect(await controller.presentationOptions(channel: "room", thread: nil).contains(.banner),
                "A room notification still appears while its collapsed thread is open.")
        #expect(await controller.presentationOptions(channel: "room", thread: "thread-2").contains(.banner))
        #expect(ThreadHTTPStub.requests.withLock { $0 }.allSatisfy { !$0.path.hasPrefix("/messages/") },
                "A signed thread hint must route immediately without a message lookup.")
    }

    @Test @MainActor func roomRequestsUseRootReadsWhileExplicitMarkAllRetainsFlatSemantics() async throws {
        ThreadHTTPStub.reset()
        let store = serviceStore()
        defer { store.service?.close() }
        store.threadMetadata["thread-1"] = thread()

        try await store.loadMessages(channelId: "room")
        try await store.markRead(channelId: "room", messageId: "root")
        try await store.markAllRead(channelId: "room")

        #expect(store.messages["room"]?.map(\.id) == ["root"],
                "A defensive filter keeps a misbehaving server from flattening replies into the room.")
        let requests = ThreadHTTPStub.requests.withLock { $0 }
        let messageGets = requests.filter { $0.method == "GET" && $0.path == "/channels/room/messages" }
        #expect(messageGets.count == 2)
        #expect(messageGets[0].query?.contains("roots_only=true") == true)
        #expect(messageGets[1].query?.contains("roots_only") == false)
        let reads = requests.filter { $0.method == "PUT" && $0.path == "/channels/room/read" }
        #expect(reads.count == 2)
        #expect(try body(reads[0])["roots_only"] as? Bool == true)
        #expect(try body(reads[1])["roots_only"] == nil)
        #expect(store.threadReadStates["thread-1"]?.lastReadId == "reply-1",
                "Flat mark-all refreshes the known thread read after the server advances it.")
    }

    @Test @MainActor func deniedThreadLoadLeavesVisibleCachedConversationIntact() async throws {
        ThreadHTTPStub.reset()
        ThreadHTTPStub.deniedPaths.withLock { $0 = ["/threads/thread-1/messages"] }
        let store = serviceStore()
        defer { store.service?.close() }
        store.threadMessages["thread-1"] = [message(id: "cached-reply", threadId: "thread-1")]

        do {
            try await store.loadThreadMessages(id: "thread-1")
            Issue.record("A denied thread request must fail.")
        } catch {
            #expect(DenFailure.present(error) == "Forbidden")
        }
        #expect(store.threadMessages["thread-1"]?.map(\.id) == ["cached-reply"])
    }

    @Test func missingThreadPositionStartsUnreadAtTheFirstForeignReply() {
        let replies = [message(id: "reply-1"), message(id: "reply-2")]

        #expect(MessagePresentation.firstUnread(in: replies, after: nil, excluding: "me") == "reply-1")
    }

    @Test @MainActor func channelSnapshotDoesNotInvalidateAnInFlightThreadRead() {
        let store = AppStore()
        let token = store.readOrder.begin(.thread("thread-1"))

        store.replaceReadStates([.init(channelId: "room", lastReadId: "root", mentionCount: 0,
                                       notificationCount: 0, unreadCount: 0)])
        store.applyThreadRead(read(last: "reply-1", unread: 0), token: token)

        #expect(store.threadReadStates["thread-1"]?.lastReadId == "reply-1")
    }

    @Test @MainActor func missingPushThreadHintStillRoutesACachedReplyToItsThread() async {
        let store = AppStore()
        store.user = .init(bot: false, displayName: "Me", id: "me", role: .member, username: "me")
        store.threadMessages["thread-1"] = [message(id: "reply-2", threadId: "thread-1")]
        let controller = NotificationController(store: store,
            authorizationStatus: { .denied }, requestAuthorization: { false }, registerRemote: {})

        await controller.handleTap(channel: "room", message: "reply-2")

        #expect(store.selectedThread?.threadId == "thread-1")
        store.selectChannel("room")
        #expect(await controller.presentationOptions(channel: "room", message: "reply-2", thread: nil).contains(.banner))
    }

    @Test @MainActor func metadataBindsARootOnlySelectionToTheDiscoveredThread() {
        let store = AppStore()
        store.selectThread(channelId: "room", rootId: "root")

        store.applyThreadMetadata(thread())

        #expect(store.selectedThread?.threadId == "thread-1")
    }

    @Test @MainActor func realtimeUpdatesARootHeldOnlyByTheThreadCache() async throws {
        let store = AppStore()
        store.channels = [.init(id: "room", kind: .text, memberIds: [], name: "Room", position: 0)]
        store.threadRoots["root"] = message(id: "root")
        var edited = try #require(try JSONSerialization.jsonObject(with:
            JSONEncoder().encode(message(id: "root", content: "Edited"))) as? [String: Any])
        edited["type"] = "message_edited"

        try await store.receive(try JSONSerialization.data(withJSONObject: edited))
        try await store.receive(try JSONSerialization.data(withJSONObject: [
            "type": "reactions_updated", "channel_id": "room", "message_id": "root",
            "reactions": [["emoji": "👍", "user_ids": ["alice"]]],
        ]))

        #expect(store.threadRoots["root"]?.content == "Edited")
        #expect(store.threadRoots["root"]?.reactions?.first?.emoji == "👍")
    }

    @Test @MainActor func cachedReplyReferenceStaysInsideItsThread() {
        let store = AppStore()
        let parent = message(id: "reply-1", threadId: "thread-1")
        let child = message(id: "reply-2", threadId: "thread-1")
        store.threadMessages["thread-1"] = [parent, child]
        store.selectThread(channelId: "room", rootId: "root", threadId: "thread-1")

        #expect(store.cachedMessage(id: parent.id, channelId: "room")?.id == parent.id)
        store.openReplyReference(from: child, parentId: parent.id)

        #expect(store.selectedThread?.threadId == "thread-1")
        #expect(store.selectedThread?.targetMessageId == parent.id)
    }

    @Test @MainActor func offPageReplyReferenceLoadsAWindowContainingItsParent() async throws {
        ThreadHTTPStub.reset()
        let store = serviceStore()
        defer { store.service?.close() }
        store.threadMessages["thread-1"] = [message(id: "reply-200", threadId: "thread-1")]

        let available = try await store.prepareThreadReference(
            threadId: "thread-1", parentId: "reply-050")

        #expect(available)
        #expect(store.threadMessages["thread-1"]?.map(\.id) == ["reply-049", "reply-050", "reply-051"])
        let requests = ThreadHTTPStub.requests.withLock { $0 }
        #expect(requests.contains { $0.path == "/messages/reply-050" })
        #expect(requests.contains { $0.path == "/threads/thread-1/messages" &&
            $0.query?.contains("before=reply-050") == true })
        #expect(requests.contains { $0.path == "/threads/thread-1/messages" &&
            $0.query?.contains("after=reply-050") == true })
    }

    @Test @MainActor func threadRootReplyReferenceIsAvailableWithoutANetworkLookup() async throws {
        ThreadHTTPStub.reset()
        let store = serviceStore()
        defer { store.service?.close() }
        store.threadMetadata["thread-1"] = thread()
        store.threadRoots["root"] = message(id: "root")

        #expect(try await store.prepareThreadReference(threadId: "thread-1", parentId: "root"))
        #expect(ThreadHTTPStub.requests.withLock { $0 }.isEmpty)
    }

    @Test @MainActor func cachedThreadRootReplyReferenceRemainsAvailableOffline() async throws {
        ThreadHTTPStub.reset()
        let store = serviceStore()
        defer { store.service?.close() }
        store.threadMetadata["thread-1"] = thread()
        store.threadRoots["root"] = message(id: "root")
        store.syncProblem = .networkOffline

        #expect(try await store.prepareThreadReference(threadId: "thread-1", parentId: "root"))
        #expect(ThreadHTTPStub.requests.withLock { $0 }.isEmpty)
    }

    @Test @MainActor func cachedThreadHydrationIsQuietWhileOffline() async throws {
        ThreadHTTPStub.reset()
        let store = serviceStore()
        defer { store.service?.close() }
        store.syncProblem = .networkOffline
        store.threadMetadata["thread-1"] = thread()
        store.threadRoots["root"] = message(id: "root")
        store.threadMessages["thread-1"] = [message(id: "cached-reply", threadId: "thread-1")]

        try await store.hydrateThread(id: "thread-1", target: nil)

        #expect(ThreadHTTPStub.requests.withLock { $0 }.isEmpty)
        #expect(store.threadMessages["thread-1"]?.map(\.id) == ["cached-reply"])
        #expect(store.error == nil)
    }

    @Test @MainActor func unreadThreadLoadingFollowsEveryPage() async throws {
        ThreadHTTPStub.reset()
        let store = serviceStore()
        defer { store.service?.close() }

        let values = try await store.loadAllThreads(channelId: "room", unreadOnly: true)

        #expect(values.count == 51)
        #expect(store.threadMetadata.count == 51)
        let requests = ThreadHTTPStub.requests.withLock { $0 }
            .filter { $0.path == "/channels/room/threads" }
        #expect(requests.count == 2)
    }

    @Test @MainActor func pruningAChannelRemovesItsThreadCacheFromMemoryAndDisk() throws {
        let store = AppStore()
        store.origin = URL(string: "https://prune-\(UUID().uuidString).test")!
        defer { OfflineCache.remove(origin: store.origin) }
        store.user = .init(bot: false, displayName: "Me", id: "me", role: .member, username: "me")
        store.threadMetadata["thread-1"] = thread()
        store.threadReadStates["thread-1"] = read(last: "reply-1", unread: 1)
        store.threadRoots["root"] = message(id: "root")
        store.threadMessages["thread-1"] = [message(id: "reply-1", threadId: "thread-1")]

        store.pruneConversationCaches(visibleChannelIds: [])
        store.saveCache()

        #expect(store.threadMetadata.isEmpty)
        #expect(store.threadReadStates.isEmpty)
        #expect(store.threadRoots.isEmpty)
        #expect(store.threadMessages.isEmpty)
        let cached = try #require(OfflineCache.read(origin: store.origin))
        #expect(cached.threadMetadata?.isEmpty == true)
        #expect(cached.threadMessages?.isEmpty == true)
    }

    @MainActor private func serviceStore() -> AppStore {
        let configuration = URLSessionConfiguration.ephemeral
        configuration.protocolClasses = [ThreadHTTPStub.self]
        let store = AppStore()
        store.origin = URL(string: "https://threads.test")!
        store.service = DenService(origin: store.origin, token: "test-only-token",
                                   sessionConfiguration: configuration)
        return store
    }

    private func thread() -> API.ThreadSummary {
        .init(channelId: "room", createdAt: "2026-09-19T12:00:00Z", createdBy: "alice",
              id: "thread-1", lastActivityAt: "2026-09-19T12:02:00Z", lastReplyId: "reply-1",
              replyCount: 1, rootMessageId: "root", title: "Thread topic")
    }

    private func read(last: String, unread: Int64) -> API.ThreadReadState {
        .init(channelId: "room", following: true, lastReadId: last, mentionCount: 0,
              notificationCount: unread, threadId: "thread-1", unreadCount: unread)
    }

    private func message(id: String, threadId: String? = nil, content: String? = nil) -> API.Message {
        .init(attachments: [], authorId: "alice", channelId: "room", content: content ?? id,
              createdAt: "2026-09-19T12:01:00Z", id: id, threadId: threadId)
    }

    private func body(_ request: ThreadHTTPStub.Request) throws -> [String: Any] {
        try #require(try JSONSerialization.jsonObject(with: request.body) as? [String: Any])
    }
}

private final class ThreadHTTPStub: URLProtocol, @unchecked Sendable {
    struct Request: Sendable {
        let method: String
        let path: String
        let query: String?
        let body: Data
    }
    static let requests = Mutex([Request]())
    static let deniedPaths = Mutex(Set<String>())
    static func reset() {
        requests.withLock { $0.removeAll() }
        deniedPaths.withLock { $0.removeAll() }
    }
    override class func canInit(with request: URLRequest) -> Bool { request.url?.host == "threads.test" }
    override class func canonicalRequest(for request: URLRequest) -> URLRequest { request }
    override func startLoading() {
        let body = Self.requestBody(request)
        let path = request.url!.path
        Self.requests.withLock {
            $0.append(.init(method: request.httpMethod ?? "", path: path,
                            query: request.url?.query, body: body))
        }
        let denied = Self.deniedPaths.withLock { $0.contains(path) }
        let status = denied ? 403 : 200
        let responseBody = denied
            ? Data(#"{"error":"forbidden","message":"Forbidden"}"#.utf8)
            : Self.response(path: path, method: request.httpMethod ?? "", query: request.url?.query)
        let response = HTTPURLResponse(url: request.url!, statusCode: status, httpVersion: "HTTP/1.1",
                                       headerFields: ["Content-Type": "application/json"])!
        client?.urlProtocol(self, didReceive: response, cacheStoragePolicy: .notAllowed)
        client?.urlProtocol(self, didLoad: responseBody)
        client?.urlProtocolDidFinishLoading(self)
    }
    override func stopLoading() {}

    private static func response(path: String, method: String, query: String?) -> Data {
        switch (method, path) {
        case ("GET", "/channels/room/messages"):
            return Data((query?.contains("roots_only=true") == true ? rootAndReply : rootAndReply).utf8)
        case ("PUT", "/channels/room/read"):
            return Data(channelRead.utf8)
        case ("GET", "/threads/thread-1"):
            return Data(threadView.utf8)
        case ("GET", "/threads/thread-1/messages"):
            if query?.contains("before=reply-050") == true { return Data("[\(reply049)]".utf8) }
            if query?.contains("after=reply-050") == true { return Data("[\(reply051)]".utf8) }
            return Data("[\(reply200)]".utf8)
        case ("GET", "/messages/reply-050"):
            return Data(reply050.utf8)
        case ("GET", "/channels/room/threads"):
            let values = query?.contains("before=thread-050") == true
                ? [threadView(index: 51)]
                : (1...50).map(threadView(index:))
            return try! JSONSerialization.data(withJSONObject: values)
        case ("GET", "/users/me/read-state"):
            return Data("[\(channelRead)]".utf8)
        default:
            return Data("[]".utf8)
        }
    }

    private static func requestBody(_ request: URLRequest) -> Data {
        if let body = request.httpBody { return body }
        guard let stream = request.httpBodyStream else { return Data() }
        stream.open(); defer { stream.close() }
        var data = Data(), buffer = [UInt8](repeating: 0, count: 4096)
        while true {
            let count = stream.read(&buffer, maxLength: buffer.count)
            guard count > 0 else { break }
            data.append(buffer, count: count)
        }
        return data
    }

    private static let root = #"{"id":"root","channel_id":"room","author_id":"alice","content":"Root","created_at":"2026-09-19T12:00:00Z","attachments":[]}"#
    private static let reply = #"{"id":"reply-1","channel_id":"room","author_id":"alice","content":"Reply","created_at":"2026-09-19T12:01:00Z","attachments":[],"thread_id":"thread-1"}"#
    private static let reply049 = #"{"id":"reply-049","channel_id":"room","author_id":"alice","content":"Earlier","created_at":"2026-09-19T12:01:00Z","attachments":[],"thread_id":"thread-1"}"#
    private static let reply050 = #"{"id":"reply-050","channel_id":"room","author_id":"alice","content":"Quoted parent","created_at":"2026-09-19T12:02:00Z","attachments":[],"thread_id":"thread-1"}"#
    private static let reply051 = #"{"id":"reply-051","channel_id":"room","author_id":"alice","content":"Later","created_at":"2026-09-19T12:03:00Z","attachments":[],"thread_id":"thread-1"}"#
    private static let reply200 = #"{"id":"reply-200","channel_id":"room","author_id":"alice","content":"Tail","created_at":"2026-09-19T12:04:00Z","attachments":[],"thread_id":"thread-1"}"#
    private static let rootAndReply = "[\(root),\(reply)]"
    private static let summary = #"{"id":"thread-1","channel_id":"room","root_message_id":"root","title":"Thread topic","created_by":"alice","created_at":"2026-09-19T12:00:00Z","last_activity_at":"2026-09-19T12:01:00Z","last_reply_id":"reply-1","reply_count":1}"#
    private static let threadRead = #"{"channel_id":"room","thread_id":"thread-1","last_read_id":"reply-1","unread_count":0,"mention_count":0,"notification_count":0,"following":true}"#
    private static let threadView = "{\"thread\":\(summary),\"read_state\":\(threadRead)}"
    private static let channelRead = #"{"channel_id":"room","last_read_id":"reply-1","unread_count":0,"mention_count":0,"notification_count":0}"#

    private static func threadView(index: Int) -> [String: Any] {
        let id = String(format: "thread-%03d", index)
        let root = String(format: "root-%03d", index)
        let reply = String(format: "reply-%03d", index)
        return [
            "thread": ["id": id, "channel_id": "room", "root_message_id": root,
                       "title": "Thread \(index)", "created_by": "alice",
                       "created_at": "2026-09-19T12:00:00Z", "last_activity_at": "2026-09-19T12:01:00Z",
                       "last_reply_id": reply, "reply_count": 1],
            "read_state": ["channel_id": "room", "thread_id": id, "last_read_id": NSNull(),
                           "unread_count": 1, "mention_count": 0, "notification_count": 1,
                           "following": true],
        ]
    }
}
