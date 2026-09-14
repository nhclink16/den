import DenAPI
import Foundation
import SwiftUI
import Testing
@testable import Den

struct SessionAndMessagesTests {
    @Test func canonicalOriginsShareOneSessionIdentity() throws {
        let cases = [
            ("  HTTPS://DENCHAT.APP:443/\n", "https://denchat.app"),
            ("https://denchat.app", "https://denchat.app"),
            ("https://denchat.app:8443/", "https://denchat.app:8443"),
            ("http://LOCALHOST:80/", "http://localhost"),
            ("http://127.0.0.1:7000/", "http://127.0.0.1:7000"),
            ("http://[::1]:7000/", "http://[::1]:7000"),
        ]
        for (input, expected) in cases {
            let origin = try ServerOrigin.canonical(input)
            #expect(origin.absoluteString == expected, "Input: \(input)")
            #expect(try ServerOrigin.canonical(origin.absoluteString) == origin)
        }
    }

    @Test func rejectsCredentialsAndAddressesThatAreNotSafeOrigins() {
        for address in [
            "https://alice:secret@denchat.app",
            "https://alice@denchat.app",
            "https://denchat.app@other.example",
            "https://denchat.app/room",
            "https://denchat.app/%2f",
            "https://denchat.app?next=/room",
            "https://denchat.app#room",
            "http://denchat.app",
            "http://localhost.other.example",
            "http://127.0.0.1.other.example",
            "ftp://denchat.app",
            "denchat.app",
        ] {
            #expect(throws: DenFailure.invalidOrigin, "Address: \(address)") {
                try ServerOrigin.canonical(address)
            }
        }
    }

    @Test func groupingStopsAtTimeAuthorRoomAndReplyBoundaries() {
        let previous = message(at: "2026-09-14T12:00:00Z")
        let justInside = message(at: "2026-09-14T12:04:59.999Z")
        #expect(MessagePresentation.groups(justInside, after: previous, calendar: utc))
        #expect(MessagePresentation.groups(message(at: previous.createdAt), after: previous, calendar: utc))
        #expect(!MessagePresentation.groups(message(at: "2026-09-14T12:05:00Z"), after: previous, calendar: utc))
        #expect(!MessagePresentation.groups(message(at: "2026-09-14T11:59:59Z"), after: previous, calendar: utc))
        #expect(!MessagePresentation.groups(justInside, after: nil, calendar: utc))
        #expect(!MessagePresentation.groups(message(at: "invalid"), after: previous, calendar: utc))

        var changed = justInside
        changed.authorId = "other-author"
        #expect(!MessagePresentation.groups(changed, after: previous, calendar: utc))
        changed = justInside
        changed.channelId = "other-room"
        #expect(!MessagePresentation.groups(changed, after: previous, calendar: utc))
        changed = justInside
        changed.replyTo = previous.id
        #expect(!MessagePresentation.groups(changed, after: previous, calendar: utc))
    }

    @Test func localMidnightStartsANewDayEvenOneSecondLater() throws {
        var newYork = Calendar(identifier: .gregorian)
        newYork.timeZone = try #require(TimeZone(identifier: "America/New_York"))
        let before = message(at: "2026-09-14T03:59:59Z")
        let after = message(at: "2026-09-14T04:00:00Z")
        #expect(MessagePresentation.startsDay(after, after: before, calendar: newYork))
        #expect(!MessagePresentation.groups(after, after: before, calendar: newYork))
        #expect(!MessagePresentation.startsDay(after, after: before, calendar: utc))
        #expect(MessagePresentation.groups(after, after: before, calendar: utc))
        #expect(MessagePresentation.startsDay(after, after: nil, calendar: newYork))
        #expect(MessagePresentation.startsDay(after, after: message(at: "invalid"), calendar: newYork))
    }

    @Test func mentionsUseUnicodeRangesAndWholeKnownUsernames() {
        let longest = String(repeating: "a", count: 32)
        let users = [user("nico", name: "Nicolás"), user("sam", name: ""), user(longest, name: "Long name")]
        let text = "👩🏽‍💻 Hello @nico, café@nico @unknown @NICO @nico_extra @sam @\(longest) @\(longest)a"
        let mentions = MessagePresentation.mentions(in: text, users: users)
        #expect(mentions.map(\.userId) == ["user-nico", "user-sam", "user-\(longest)"])
        #expect(mentions.map(\.displayName) == ["Nicolás", "sam", "Long name"])
        #expect(mentions.map { (text as NSString).substring(with: $0.range) } == ["@nico", "@sam", "@\(longest)"])
        #expect(mentions.first?.range == (text as NSString).range(of: "@nico"))
    }

    @Test @MainActor func markdownMentionsPreserveEmphasisButLeaveCodeAndLinksLiteral() throws {
        let theme = DenTheme.defaultTheme(scheme: .dark)
        let value = MessagePresentation.markdown(
            "**Hi @nico** and `@nico` and [@nico](https://example.test/@nico).",
            users: [user("nico", name: "Nicolás")], theme: theme
        )
        #expect(String(value.characters) == "Hi @Nicolás and @nico and @nico.")
        let mention = try #require(value.runs.first { String(value[$0.range].characters) == "@Nicolás" })
        #expect(mention.inlinePresentationIntent?.contains(.stronglyEmphasized) == true)
        #expect(mention.foregroundColor == theme.accent)
        #expect(mention.backgroundColor == theme.accentGlow)
        let code = try #require(value.runs.first { $0.inlinePresentationIntent?.contains(.code) == true })
        #expect(String(value[code.range].characters) == "@nico")
        let link = try #require(value.runs.first { $0.link != nil })
        #expect(String(value[link.range].characters) == "@nico")
        #expect(link.link?.absoluteString == "https://example.test/@nico")
    }

    @Test func fencedCodeDoesNotBecomeAMentionOrShowTheLanguageLabel() {
        let blocks = MessagePresentation.blocks("Before @nico\n```swift\nlet name = \"@nico\"\n```\nAfter")
        #expect(blocks.map(\.isCode) == [false, true, false])
        #expect(blocks.map(\.text) == ["Before @nico\n", "let name = \"@nico\"", "\nAfter"])
        let unfinished = MessagePresentation.blocks("```txt\n@nico")
        #expect(unfinished.count == 1)
        #expect(unfinished.first?.isCode == true)
        #expect(unfinished.first?.text == "@nico")
    }

    private var utc: Calendar {
        var calendar = Calendar(identifier: .gregorian)
        calendar.timeZone = TimeZone(secondsFromGMT: 0)!
        return calendar
    }

    private func message(at timestamp: String) -> Components.Schemas.Message {
        .init(attachments: [], authorId: "author", channelId: "room", content: "Hello",
              createdAt: timestamp, id: "message-\(timestamp)")
    }

    private func user(_ username: String, name: String) -> Components.Schemas.User {
        .init(bot: false, displayName: name, id: "user-\(username)", role: .member, username: username)
    }
}
