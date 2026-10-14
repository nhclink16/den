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
