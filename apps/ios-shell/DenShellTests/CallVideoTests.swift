import Foundation
import LiveKit
import Testing
import UIKit
@testable import DenShell

// The page places call video by naming whose camera or share goes where. Only well-formed slots pass, the names must
// match the page's own (shell-video.ts `videoKey`), and the tile's colour must come through for a fitted share.
struct CallVideoTests {
    let slot: [String: Any] = ["key": "u1:a|camera", "rect": ["x": 12, "y": 96.5, "width": 378, "height": 212.6], "radius": 14,
                               "fill": "rgb(40, 40, 44)", "fit": "cover", "identity": "u1:a", "source": "camera"]

    @Test func readsTheSlots() throws {
        let command = try VideoCommand(["type": "slots", "seq": 7, "slots": [slot]])
        #expect(command.seq == 7)
        let only = try #require(command.slots.first)
        #expect(only.key == "u1:a|camera")
        #expect(only.rect == CGRect(x: 12, y: 96.5, width: 378, height: 212.6))
        #expect(only.radius == 14)
        #expect(only.fit == .cover)
        #expect(try VideoCommand(["type": "slots", "seq": 8, "slots": []]).slots.isEmpty, "no slots: put every video away")
    }

    @Test func refusesTheRest() {
        #expect(throws: CallError.self) { try VideoCommand(["type": "slots", "slots": [slot]]) }
        #expect(throws: CallError.self) { try VideoCommand(["type": "slots", "seq": true, "slots": [slot]]) }
        #expect(throws: CallError.self) { try VideoCommand(["type": "place", "seq": 1, "slots": [slot]]) }
        for key in ["key", "rect", "radius", "fit"] {
            var broken = slot; broken.removeValue(forKey: key)
            #expect(throws: CallError.self, "without \(key)") { try VideoCommand(["type": "slots", "seq": 1, "slots": [broken]]) }
        }
        var flat = slot; flat["rect"] = ["x": 0, "y": 0, "width": 378, "height": 0]
        #expect(throws: CallError.self, "an empty tile") { try VideoSlot(flat) }
        var odd = slot; odd["fit"] = "stretch"
        #expect(throws: CallError.self) { try VideoSlot(odd) }
        #expect(throws: CallError.self, "no more slots than a call has tiles") { try VideoCommand(["type": "slots", "seq": 1, "slots": Array(repeating: slot, count: 40)]) }
    }

    @Test func cornersNoRounderThanTheTile() throws {
        var round = slot; round["radius"] = 999
        #expect(try VideoSlot(round).radius == 106.3)
    }

    @Test func readsTheTileColour() {
        var (r, g, b, a): (CGFloat, CGFloat, CGFloat, CGFloat) = (0, 0, 0, 0)
        #expect(VideoSlot.color("rgb(40, 40, 44)")?.getRed(&r, green: &g, blue: &b, alpha: &a) == true)
        #expect(abs(r - 40 / 255) < 0.001 && abs(b - 44 / 255) < 0.001 && a == 1)
        #expect(VideoSlot.color("rgba(255, 0, 0, 0.5)")?.getRed(&r, green: &g, blue: &b, alpha: &a) == true)
        #expect(a == 0.5)
        #expect(VideoSlot.color("color(display-p3 1 0 0)") == nil, "not one it can read: the view's own black")
        #expect(VideoSlot.color("rgb(300, 0, 0)") == nil)
    }

    @Test func namesVideoAsThePageDoes() {
        #expect(CallRules.videoKey(identity: "u1:a", source: .camera, name: "camera") == "u1:a|camera")
        #expect(CallRules.videoKey(identity: "u1:a", source: .screenShareVideo, name: "screen-2") == "u1:a|screen|screen-2")
        #expect(CallRules.videoKey(identity: "u1:a", source: .microphone, name: "mic") == nil)
        #expect(CallRules.videoKey(identity: "u1:a", source: .screenShareAudio, name: "screen-2-audio") == nil)
    }
}
