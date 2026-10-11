import Foundation
import Testing
@testable import DenShell

// The origin check decides who gets the camera and microphone, so it must not match lookalikes.
struct OriginTests {
    let den = Origin(URL(string: "https://denchat.app")!)!

    @Test func sameOriginMatchesWithOrWithoutDefaultPort() {
        #expect(Origin(scheme: "https", host: "denchat.app", port: 0) == den)
        #expect(Origin(scheme: "https", host: "denchat.app", port: 443) == den)
        #expect(Origin(scheme: "HTTPS", host: "DenChat.App", port: 0) == den)
        #expect(Origin(URL(string: "https://denchat.app/channels/abc?x=1")!) == den)
    }

    @Test func lookalikesDoNotMatch() {
        #expect(Origin(scheme: "http", host: "denchat.app", port: 0) != den)
        #expect(Origin(scheme: "https", host: "denchat.app", port: 8443) != den)
        #expect(Origin(scheme: "https", host: "evil.denchat.app", port: 0) != den)
        #expect(Origin(scheme: "https", host: "denchat.app.evil.com", port: 0) != den)
        #expect(Origin(URL(string: "https://denchat.app@evil.com/")!) != den)
    }

    @Test func nonWebSchemesHaveNoOrigin() {
        #expect(Origin(scheme: "file", host: "denchat.app", port: 0) == nil)
        #expect(Origin(scheme: "https", host: "", port: 0) == nil)
        #expect(Origin(URL(string: "about:blank")!) == nil)
        #expect(Origin(URL(string: "data:text/html,hi")!) == nil)
    }

    @Test func blobTakesTheOriginOfItsMaker() {
        #expect(Origin(URL(string: "blob:https://denchat.app/1b6e1c1e-0f1a-4c8e-9d1e-3f0c2a1b2c3d")!) == den)
        #expect(Origin(URL(string: "blob:https://evil.com/1b6e1c1e-0f1a-4c8e-9d1e-3f0c2a1b2c3d")!) != den)
        #expect(Origin(URL(string: "blob:null/1b6e1c1e")!) == nil)
    }

    @Test func testServerPortIsPartOfTheOrigin() {
        let test = Origin(URL(string: "https://box.example.ts.net:4203")!)!
        #expect(Origin(scheme: "https", host: "box.example.ts.net", port: 4203) == test)
        #expect(Origin(scheme: "https", host: "box.example.ts.net", port: 0) != test)
    }
}
