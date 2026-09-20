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
        let store = AppStore()
        let controller = NotificationController(store: store,
            authorizationStatus: { .denied }, requestAuthorization: { false }, registerRemote: {})
        controller.handleTap(channel: "room", message: "reply-2", thread: "thread-1")
        #expect(store.selectedChannelId == nil)
        store.user = .init(bot: false, displayName: "Me", id: "me", role: .member, username: "me")
        await controller.requestAfterLogin()

        #expect(store.selectedChannelId == "room")
        #expect(store.selectedThread == .init(channelId: "room", rootId: nil, threadId: "thread-1",
                                              targetMessageId: "reply-2"))
        #expect(controller.presentationOptions(channel: "room", thread: "thread-1") == [.badge])
        #expect(controller.presentationOptions(channel: "room", thread: nil).contains(.banner),
                "A room notification still appears while its collapsed thread is open.")
        #expect(controller.presentationOptions(channel: "room", thread: "thread-2").contains(.banner))
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

    private func message(id: String, threadId: String? = nil) -> API.Message {
        .init(attachments: [], authorId: "alice", channelId: "room", content: id,
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
            return Data("[\(reply)]".utf8)
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
    private static let rootAndReply = "[\(root),\(reply)]"
    private static let summary = #"{"id":"thread-1","channel_id":"room","root_message_id":"root","title":"Thread topic","created_by":"alice","created_at":"2026-09-19T12:00:00Z","last_activity_at":"2026-09-19T12:01:00Z","last_reply_id":"reply-1","reply_count":1}"#
    private static let threadRead = #"{"channel_id":"room","thread_id":"thread-1","last_read_id":"reply-1","unread_count":0,"mention_count":0,"notification_count":0,"following":true}"#
    private static let threadView = "{\"thread\":\(summary),\"read_state\":\(threadRead)}"
    private static let channelRead = #"{"channel_id":"room","last_read_id":"reply-1","unread_count":0,"mention_count":0,"notification_count":0}"#
}
