import Foundation
import Testing
@testable import DenShell

// A notification's payload and the page's messages both come from outside the app. A tap must only ever open a Den
// id, and a malformed message must never set the badge or reach iOS.
struct PushBridgeTests {
    @Test func readsTheServersAlertPayload() {
        let info: [AnyHashable: Any] = ["aps": ["alert": ["title": "You were mentioned in Den"]], "type": "notification",
                                        "channel_id": "01J9ZQ3K8M2N4P6R8T0V2X4Z6A", "message_id": "01J9ZQ3K8M2N4P6R8T0V2X4Z6B",
                                        "thread_id": "01J9ZQ3K8M2N4P6R8T0V2X4Z6C", "reason": "mention"]
        #expect(PushTap(info) == PushTap(channelId: "01J9ZQ3K8M2N4P6R8T0V2X4Z6A", messageId: "01J9ZQ3K8M2N4P6R8T0V2X4Z6B",
                                         threadId: "01J9ZQ3K8M2N4P6R8T0V2X4Z6C"))
    }

    @Test func aCallInvitationOpensItsRoom() {
        let info: [AnyHashable: Any] = ["type": "call_invite", "channel_id": "01J9ZQ3K8M2N4P6R8T0V2X4Z6A", "expires_at": 1_760_000_000]
        #expect(PushTap(info) == PushTap(channelId: "01J9ZQ3K8M2N4P6R8T0V2X4Z6A"))
    }

    @Test func onlyDenIdsReachThePage() {
        #expect(PushTap([:]) == nil)
        #expect(PushTap(["channel_id": 42]) == nil)
        #expect(PushTap(["channel_id": ""]) == nil)
        #expect(PushTap(["channel_id": "ABC/../settings"]) == nil, "a path")
        #expect(PushTap(["channel_id": "ABC?m=1"]) == nil, "a query")
        #expect(PushTap(["channel_id": "abc"]) == nil, "Den ids are upper case")
        #expect(PushTap(["channel_id": "ÄBC"]) == nil)
        #expect(PushTap(["channel_id": String(repeating: "A", count: 65)]) == nil)
        #expect(PushTap(["channel_id": "ABC", "message_id": "x/y", "thread_id": 7]) == PushTap(channelId: "ABC"), "a bad extra is dropped, not the tap")
    }

    @Test func readsThePagesMessages() throws {
        #expect(try PushCommand(["type": "register"]) == .register)
        #expect(try PushCommand(["type": "status"]) == .status)
        #expect(try PushCommand(["type": "take"]) == .take)
        #expect(try PushCommand(["type": "signedOut"]) == .signedOut)
        #expect(try PushCommand(["type": "settings"]) == .openSettings)
        #expect(try PushCommand(["type": "badge", "count": 3]) == .badge(3))
        #expect(try PushCommand(["type": "badge", "count": 0]) == .badge(0))
        #expect(try PushCommand(["type": "badge", "count": 123_456]) == .badge(9999), "iOS shows four digits at most")
    }

    @Test func refusesMalformedMessages() {
        #expect(throws: CallError.self) { try PushCommand("register") }
        #expect(throws: CallError.self) { try PushCommand(["type": "subscribe"]) }
        #expect(throws: CallError.self) { try PushCommand(["type": "badge"]) }
        #expect(throws: CallError.self) { try PushCommand(["type": "badge", "count": -1]) }
        #expect(throws: CallError.self) { try PushCommand(["type": "badge", "count": 1.5]) }
        #expect(throws: CallError.self, "a yes/no is not a number") { try PushCommand(["type": "badge", "count": true]) }
        #expect(throws: CallError.self) { try PushCommand(["type": "badge", "count": "2"]) }
    }
}

// A reply typed into a notification is sent by the shell itself, with the page's sign-in. It must go only to Den, only
// from the account the notification was for, and to the conversation the notification came from.
struct QuickReplyTests {
    let den = Origin(URL(string: "https://denchat.app")!)!
    var room: [AnyHashable: Any] {
        ["aps": ["category": "message"], "type": "notification", "channel_id": "ROOM1", "message_id": "MSG1",
         "reason": "mention", "to_user_id": "ELLIS"]
    }
    var thread: [AnyHashable: Any] { room.merging(["thread_id": "THREAD1", "message_id": "MSG2"]) { $1 } }

    func body(_ request: URLRequest) -> NSDictionary? {
        request.httpBody.flatMap { try? JSONSerialization.jsonObject(with: $0) as? NSDictionary }
    }

    @Test func goesToTheRoomOrTheThread() throws {
        let toRoom = try #require(QuickReply(room, text: "on my way"))
        #expect(toRoom == QuickReply(to: PushTap(channelId: "ROOM1", messageId: "MSG1"), userId: "ELLIS", text: "on my way"))
        #expect(body(toRoom.post(den: den, session: "ab")) == ["content": "on my way"], "a room reply is not a thread reply")
        let toThread = try #require(QuickReply(thread, text: "agreed"))
        let post = toThread.post(den: den, session: "ab")
        #expect(post.url?.path() == "/channels/ROOM1/messages")
        #expect(body(post) == ["content": "agreed", "thread_id": "THREAD1"])
    }

    @Test func refusesABadPayload() {
        #expect(QuickReply(room.merging(["to_user_id": ""]) { $1 }, text: "hi") == nil)
        #expect(QuickReply(room.filter { $0.key as? String != "to_user_id" }, text: "hi") == nil, "no account named")
        #expect(QuickReply(room.merging(["to_user_id": "ellis/../x"]) { $1 }, text: "hi") == nil)
        #expect(QuickReply(room.merging(["channel_id": "ROOM1/../users"]) { $1 }, text: "hi") == nil)
        #expect(QuickReply(room.merging(["type": "call_invite"]) { $1 }, text: "hi") == nil)
        #expect(QuickReply(room.merging(["thread_id": "T?x=1"]) { $1 }, text: "hi") == nil, "a thread reply never becomes a room post")
        #expect(QuickReply(room.filter { $0.key as? String != "message_id" }, text: "hi") == nil)
    }

    @Test func requestsGoToDenOnlyWithTheSession() throws {
        let reply = try #require(QuickReply(thread, text: "agreed"))
        let test = Origin(URL(string: "https://box.example.ts.net:4203")!)!
        let post = reply.post(den: test, session: "ab12")
        #expect(post.url?.absoluteString == "https://box.example.ts.net:4203/channels/ROOM1/messages")
        #expect(post.httpMethod == "POST")
        #expect(post.value(forHTTPHeaderField: "Authorization") == "Bearer ab12")
        #expect(post.value(forHTTPHeaderField: "Content-Type") == "application/json")
        #expect(post.value(forHTTPHeaderField: "Cookie") == nil)
        let me = QuickReply.request("GET", "/users/me", den: den, session: "ab12")
        #expect(me.url?.absoluteString == "https://denchat.app/users/me")
        #expect(me.httpBody == nil)

        // Read through what the notification showed, not the reply: anything newer stays unread.
        let threadRead = reply.read(den: den, session: "ab12")
        #expect(threadRead.httpMethod == "PUT")
        #expect(threadRead.url?.path() == "/threads/THREAD1/read")
        #expect(body(threadRead) == ["message_id": "MSG2"])
        let roomRead = try #require(QuickReply(room, text: "hi")).read(den: den, session: "ab12")
        #expect(roomRead.url?.path() == "/channels/ROOM1/read")
        #expect(body(roomRead) == ["message_id": "MSG1", "roots_only": true])

        let config = QuickReply.http.configuration
        #expect(config.httpCookieStorage == nil && !config.httpShouldSetCookies && config.urlCache == nil)
    }

    @Test func usesOnlyDensLiveSessionCookie() throws {
        func cookie(name: String = "den_session", value: String = "0a1b2c", domain: String = "denchat.app",
                    secure: Bool = true, expires: Date? = .now.addingTimeInterval(3600)) throws -> HTTPCookie {
            var properties: [HTTPCookiePropertyKey: Any] = [.name: name, .value: value, .domain: domain, .path: "/"]
            if secure { properties[.secure] = "TRUE" }
            if let expires { properties[.expires] = expires }
            return try #require(HTTPCookie(properties: properties))
        }
        let jar = try [cookie(name: "other"), cookie()]
        #expect(QuickReply.session(in: jar, den: den) == "0a1b2c")
        #expect(QuickReply.session(in: [], den: den) == nil, "signed out: the server's logout removed it")
        let refused = try [
            cookie(expires: .now.addingTimeInterval(-1)),
            cookie(domain: "evil.denchat.app"),
            cookie(domain: "denchat.app.evil.com"),
            cookie(domain: ".denchat.app"), // Den's cookie is host-only
            cookie(secure: false),
            cookie(value: "not-hex"), // it goes into a header
        ]
        for other in refused { #expect(QuickReply.session(in: [other], den: den) == nil, "\(other)") }
        let local = Origin(URL(string: "http://localhost:7000")!)!
        let test = try cookie(domain: "localhost", secure: false)
        #expect(QuickReply.session(in: [test], den: local) == "0a1b2c")
    }

    /// Answers like Den, signed in as `me`, and remembers what was asked.
    actor FakeDen {
        var asked: [String] = []
        let me: String
        let fail: [String: Int]
        let drop: [String: URLError.Code]

        init(me: String = "ELLIS", fail: [String: Int] = [:], drop: [String: URLError.Code] = [:]) {
            self.me = me
            self.fail = fail
            self.drop = drop
        }

        func answer(_ request: URLRequest) throws -> (Data, Int) {
            let step = "\(request.httpMethod!) \(request.url!.path())"
            asked.append(step)
            if let code = drop[step] { throw URLError(code) }
            if let status = fail[step] { return (Data(#"{"error":"x","message":"x"}"#.utf8), status) }
            let json: Any
            switch step {
            case "GET /users/me": json = ["id": me]
            case "POST /channels/ROOM1/messages": json = ["id": "MINE"]
            case "GET /users/me/read-state": json = [["notification_count": 2], ["notification_count": 1]]
            default: json = [String: String]()
            }
            return (try JSONSerialization.data(withJSONObject: json), 200)
        }
    }

    func send(_ info: [AnyHashable: Any], to fake: FakeDen, session: String? = "ab12") async throws -> QuickReply.Outcome {
        let reply = try #require(QuickReply(info, text: "on my way"))
        return await reply.send(den: den, session: session) { try await fake.answer($0) }
    }

    @Test func sendsMarksReadAndCountsTheBadge() async throws {
        let roomDen = FakeDen()
        #expect(try await send(room, to: roomDen) == .sent(badge: 3))
        #expect(await roomDen.asked == ["GET /users/me", "POST /channels/ROOM1/messages", "PUT /channels/ROOM1/read", "GET /users/me/read-state"])
        let threadDen = FakeDen()
        #expect(try await send(thread, to: threadDen) == .sent(badge: 3))
        #expect(await threadDen.asked[2] == "PUT /threads/THREAD1/read")
    }

    @Test func neverPostsFromAnotherAccount() async throws {
        let den = FakeDen(me: "MAYA")
        #expect(try await send(room, to: den) == .notSent(.otherAccount))
        #expect(await den.asked == ["GET /users/me"])
    }

    @Test func signedOutSendsNothing() async throws {
        let none = FakeDen()
        #expect(try await send(room, to: none, session: nil) == .notSent(.signedOut))
        #expect(await none.asked.isEmpty)
        let expired = FakeDen(fail: ["GET /users/me": 401])
        #expect(try await send(room, to: expired) == .notSent(.signedOut))
        #expect(await expired.asked == ["GET /users/me"])
    }

    @Test func saysWhetherItMightHaveGone() async throws {
        #expect(try await send(room, to: FakeDen(drop: ["GET /users/me": .notConnectedToInternet])) == .notSent(.failed))
        #expect(try await send(room, to: FakeDen(drop: ["POST /channels/ROOM1/messages": .notConnectedToInternet])) == .notSent(.failed))
        #expect(try await send(room, to: FakeDen(drop: ["POST /channels/ROOM1/messages": .networkConnectionLost])) == .notSent(.unknown))
        #expect(try await send(room, to: FakeDen(drop: ["POST /channels/ROOM1/messages": .cancelled])) == .notSent(.unknown), "out of time")
        #expect(try await send(room, to: FakeDen(fail: ["POST /channels/ROOM1/messages": 400])) == .notSent(.failed))
        #expect(try await send(room, to: FakeDen(fail: ["POST /channels/ROOM1/messages": 500])) == .notSent(.unknown),
                "the server saves before it answers")
        #expect(try await send(room, to: FakeDen(fail: ["PUT /channels/ROOM1/read": 500, "GET /users/me/read-state": 500])) == .sent(badge: nil),
                "sent is sent, even if the badge can't follow")
    }
}
