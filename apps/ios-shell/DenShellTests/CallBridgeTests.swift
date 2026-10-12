import Foundation
import Testing
@testable import DenShell

// The bridge hands a LiveKit token to native code and lets the page drive the mic, so only Den's own page
// may use it, and a malformed message must never turn into a call action.
struct CallBridgeTests {
    let den = Origin(URL(string: "https://denchat.app")!)!

    @Test func onlyTheDenPageItselfMayDriveTheCall() {
        #expect(CallBridge.allows(isMainFrame: true, origin: den, den: den))
        #expect(!CallBridge.allows(isMainFrame: false, origin: den, den: den), "a frame inside Den, even a Den one")
        #expect(!CallBridge.allows(isMainFrame: true, origin: Origin(URL(string: "https://evil.denchat.app")!), den: den))
        #expect(!CallBridge.allows(isMainFrame: true, origin: Origin(URL(string: "http://denchat.app")!), den: den))
        #expect(!CallBridge.allows(isMainFrame: true, origin: nil, den: den), "about:blank, data: and file pages")
    }

    @Test func theInjectedScriptComparesAgainstTheOriginAsTheBrowserWritesIt() {
        #expect(den.serialized == "https://denchat.app")
        #expect(Origin(URL(string: "https://codexbox.example.ts.net:4211/x")!)!.serialized == "https://codexbox.example.ts.net:4211")
        #expect(Origin(URL(string: "http://localhost:80")!)!.serialized == "http://localhost")
    }

    let join: [String: Any] = ["type": "join", "url": "wss://rtc.denchat.app", "token": "t", "channelId": "c1", "title": "hangout", "micOn": true]

    @Test func readsAJoin() throws {
        let command = try CallCommand(join, allowLocalWS: false)
        #expect(command == .join(CallJoin(url: URL(string: "wss://rtc.denchat.app")!, token: "t", channelId: "c1", title: "hangout", micOn: true)))
    }

    @Test func refusesAnInsecureOrForeignCallServer() {
        var plain = join; plain["url"] = "ws://rtc.denchat.app"
        #expect(throws: CallError.self) { try CallCommand(plain, allowLocalWS: true) }
        var web = join; web["url"] = "https://rtc.denchat.app"
        #expect(throws: CallError.self) { try CallCommand(web, allowLocalWS: false) }
        var local = join; local["url"] = "ws://127.0.0.1:7880"
        #expect(throws: CallError.self) { try CallCommand(local, allowLocalWS: false) }
        #expect(throws: Never.self) { try CallCommand(local, allowLocalWS: true) }
    }

    @Test func refusesMissingOrMistypedFields() {
        for key in ["url", "token", "channelId", "title", "micOn"] {
            var broken = join; broken.removeValue(forKey: key)
            #expect(throws: CallError.self, "without \(key)") { try CallCommand(broken, allowLocalWS: false) }
        }
        var numeric = join; numeric["micOn"] = 1
        #expect(throws: CallError.self, "a number is not a yes/no") { try CallCommand(numeric, allowLocalWS: false) }
        #expect(throws: CallError.self) { try CallCommand(["type": "mute"], allowLocalWS: false) }
        #expect(throws: CallError.self) { try CallCommand(["type": "camera", "on": true], allowLocalWS: false) }
        #expect(throws: CallError.self) { try CallCommand("leave", allowLocalWS: false) }
    }

    @Test func readsTheSmallCommands() throws {
        #expect(try CallCommand(["type": "leave"], allowLocalWS: false) == .leave)
        #expect(try CallCommand(["type": "mute", "muted": true], allowLocalWS: false) == .mute(true))
        #expect(try CallCommand(["type": "sound", "off": false], allowLocalWS: false) == .sound(off: false))
        #expect(try CallCommand(["type": "state"], allowLocalWS: false) == .state)
    }

    @Test func multiDeviceRules() {
        #expect(CallRules.joinsQuietly(localIdentity: "u1:b", remoteIdentities: ["u2:a", "u1:a"]))
        #expect(!CallRules.joinsQuietly(localIdentity: "u1:b", remoteIdentities: ["u2:a", "u1:b"]))
        #expect(!CallRules.subscribes(isAudio: true, isMicrophone: true, identity: "u1:a", localIdentity: "u1:b", soundOff: false), "your other device's mic")
        #expect(CallRules.subscribes(isAudio: true, isMicrophone: false, identity: "u1:a", localIdentity: "u1:b", soundOff: false), "its shared audio")
        #expect(!CallRules.subscribes(isAudio: false, isMicrophone: false, identity: "u2:a", localIdentity: "u1:b", soundOff: false), "no video in phase 1")
        #expect(!CallRules.subscribes(isAudio: true, isMicrophone: true, identity: "u2:a", localIdentity: "u1:b", soundOff: true))
    }
}
