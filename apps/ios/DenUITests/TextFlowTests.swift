import Foundation
import XCTest

/// Exercises the real native UI against the isolated server described by the private fixture file.
/// Credentials are prefilled by a simulator-only Debug hook; no password is typed into XCTest logs.
@MainActor
final class TextFlowTests: XCTestCase {
    private var app: XCUIApplication!
    private var fixture: Fixture!
    private var fixturePath = ""

    private func configureFixture() throws {
        continueAfterFailure = false
        fixturePath = ProcessInfo.processInfo.environment["DEN_UI_FIXTURE_PATH"] ?? "/tmp/den-ios-qa.json"
        guard FileManager.default.fileExists(atPath: fixturePath) else {
            if ProcessInfo.processInfo.environment["CI"] == "TRUE" || ProcessInfo.processInfo.environment["DEN_UI_REQUIRE_FIXTURE"] == "1" {
                XCTFail("The required isolated iOS QA fixture is missing.")
                throw FixtureError.invalidLocalFixture
            }
            throw XCTSkip("The isolated iOS QA server fixture has not been provisioned.")
        }
        fixture = try JSONDecoder().decode(Fixture.self, from: Data(contentsOf: URL(fileURLWithPath: fixturePath)))
        guard let origin = URL(string: fixture.origin), ["127.0.0.1", "localhost"].contains(origin.host), fixture.users.count >= 2 else {
            throw FixtureError.invalidLocalFixture
        }
        app = XCUIApplication()
        app.launchArguments = ["--den-ui-test", "--den-ui-reset"]
        app.launchEnvironment["DEN_UI_FIXTURE_PATH"] = fixturePath
        app.launchEnvironment["DEN_UI_USER"] = "0"
        addUIInterruptionMonitor(withDescription: "Den notification permission") { alert in
            let text = alert.staticTexts.allElementsBoundByIndex.map(\.label).joined(separator: " ").lowercased()
            guard text.contains("notification"), alert.buttons["Allow"].exists else { return false }
            alert.buttons["Allow"].tap()
            return true
        }
    }

    func testNativeTextFlowAndSessionRestoration() async throws {
        try configureFixture()
        // Each run must actually change appearance, even when a prior interrupted
        // test left this disposable account on the target theme.
        let _: Appearance = try await request("/users/me/appearance", method: "PUT",
                                             body: ["theme": "den", "mode": "light", "custom_themes": []])
        let marker = "iosqa" + UUID().uuidString.replacingOccurrences(of: "-", with: "").lowercased()
        app.launch()
        XCTAssertTrue(element("login-submit").waitForExistence(timeout: 10))
        XCTAssertFalse((element("login-username").value as? String ?? "").isEmpty)
        element("login-submit").tap()
        try await waitForRooms()

        // A second process must restore the account from the Keychain, without tapping Sign in.
        app.terminate()
        app.launchArguments = ["--den-ui-test"]
        app.launch()
        try await waitForRooms()
        XCTAssertFalse(element("login-submit").exists)

        openRoom(fixture.generalChannelId)
        let original = marker + " first message"
        send(original, usingReturn: true)
        let first = try await waitForMessage(channel: fixture.generalChannelId, content: original)
        XCTAssertTrue(element("message-" + first.id).waitForExistence(timeout: 10))

        contextAction("Edit", messageId: first.id)
        let editor = element("edit-message-field")
        XCTAssertTrue(editor.waitForExistence(timeout: 5))
        editor.tap()
        editor.typeText(" revised")
        app.buttons["Save"].tap()
        let edited = try await waitForMessage(id: first.id) { $0.content != original }
        XCTAssertEqual(edited.content.replacingOccurrences(of: " revised", with: ""), original)
        XCTAssertTrue(edited.content.contains(" revised"))

        contextAction("Reply", messageId: first.id)
        XCTAssertTrue(app.staticTexts.matching(NSPredicate(format: "label BEGINSWITH %@", "Replying to")).firstMatch.waitForExistence(timeout: 5))
        let replyText = marker + " reply"
        send(replyText)
        let reply = try await waitForMessage(channel: fixture.generalChannelId, content: replyText)
        XCTAssertEqual(reply.replyTo, first.id)

        contextAction("React", messageId: first.id)
        XCTAssertTrue(app.buttons["👍"].waitForExistence(timeout: 5))
        app.buttons["👍"].tap()
        let reacted = try await waitForMessage(id: first.id) { message in
            message.reactions.contains { $0.emoji == "👍" && $0.userIds.contains(self.fixture.users[0].session.user.id) }
        }
        XCTAssertEqual(reacted.reactions.first { $0.emoji == "👍" }?.userIds.filter { $0 == fixture.users[0].session.user.id }.count, 1)
        let reaction = element("reaction-\(first.id)-👍")
        XCTAssertTrue(reaction.waitForExistence(timeout: 5))
        reaction.tap()
        _ = try await waitForMessage(id: first.id) { message in
            !message.reactions.contains { $0.emoji == "👍" && $0.userIds.contains(self.fixture.users[0].session.user.id) }
        }
        screenshot("m5a-conversation-reply")

        contextAction("Delete", messageId: first.id)
        XCTAssertTrue(app.buttons["Delete message"].waitForExistence(timeout: 5))
        app.buttons["Delete message"].tap()
        try await waitForDeletion(first.id)
        let deletedRow = XCTNSPredicateExpectation(predicate: NSPredicate(format: "exists == false"), object: element("message-" + first.id))
        XCTAssertEqual(XCTWaiter.wait(for: [deletedRow], timeout: 10), .completed)

        // Use the person picker, even though the fixture already has a DM between these two users.
        showRoomsList()
        element("new-dm").tap()
        let person = element("dm-person-" + fixture.users[1].session.user.id)
        XCTAssertTrue(person.waitForExistence(timeout: 5))
        person.tap()
        element("open-dm").tap()
        XCTAssertTrue(element("composer-field").waitForExistence(timeout: 10))
        let dmText = marker + " direct message"
        send(dmText)
        let dm = try await waitForMessage(channel: fixture.dmChannelId, content: dmText)
        XCTAssertEqual(dm.channelId, fixture.dmChannelId)

        // A second authenticated user creates an unread mention while the app is in another room.
        let incoming = marker + " inbox @" + fixture.users[0].username
        let notification = try await postMessage(channel: fixture.generalChannelId, content: incoming, user: 1)
        selectTab("Inbox")
        let inboxRoom = element("inbox-room-" + fixture.generalChannelId)
        XCTAssertTrue(inboxRoom.waitForExistence(timeout: 10))
        screenshot("m5a-inbox")
        inboxRoom.tap()
        XCTAssertTrue(element("message-" + notification.id).waitForExistence(timeout: 10))
        XCTAssertTrue(app.tabBars.buttons["Rooms"].isSelected)

        selectTab("Search")
        let search = app.searchFields.firstMatch
        XCTAssertTrue(search.waitForExistence(timeout: 5))
        search.tap()
        search.typeText(marker + "\n")
        let result = element("search-result-" + reply.id)
        XCTAssertTrue(result.waitForExistence(timeout: 10))
        result.tap()
        XCTAssertTrue(element("message-" + reply.id).waitForExistence(timeout: 10))
        XCTAssertTrue(app.tabBars.buttons["Rooms"].isSelected)

        selectTab("Settings")
        app.buttons["Appearance"].tap()
        let tide = element("theme-tide")
        XCTAssertTrue(tide.waitForExistence(timeout: 5))
        tide.tap()
        try await waitForAppearance(theme: "tide", mode: nil)
        app.buttons["Dark"].tap()
        try await waitForAppearance(theme: "tide", mode: "dark")
        screenshot("m5a-appearance-tide")
        app.navigationBars.buttons.element(boundBy: 0).tap()
        let logout = element("account-logout")
        if !logout.isHittable { app.swipeUp() }
        XCTAssertTrue(logout.waitForExistence(timeout: 5))
        logout.tap()
        let confirm = app.sheets["Sign out of Den?"].buttons["Sign out"]
        XCTAssertTrue(confirm.waitForExistence(timeout: 5))
        XCTAssertTrue(confirm.isHittable)
        confirm.tap()
        XCTAssertTrue(element("login-submit").waitForExistence(timeout: 10))
        app.terminate()
        app.launch()
        XCTAssertTrue(element("login-submit").waitForExistence(timeout: 10))
        XCTAssertFalse(app.tabBars.buttons["Rooms"].exists)
    }

    private func waitForRooms() async throws {
        let rooms = app.tabBars.buttons["Rooms"]
        XCTAssertTrue(rooms.waitForExistence(timeout: 20), "The authenticated Rooms tab must appear after login or restore.")
        // Tapping the app gives XCTest a chance to handle a system notification permission alert.
        if rooms.isHittable { rooms.tap() }
        XCTAssertTrue(element("room-" + fixture.generalChannelId).waitForExistence(timeout: 10))
    }

    private func openRoom(_ id: String) {
        let room = element("room-" + id)
        XCTAssertTrue(room.waitForExistence(timeout: 10))
        room.tap()
        XCTAssertTrue(element("composer-field").waitForExistence(timeout: 10))
    }

    private func showRoomsList() {
        selectTab("Rooms")
        if !element("new-dm").exists { app.navigationBars.buttons.element(boundBy: 0).tap() }
        XCTAssertTrue(element("new-dm").waitForExistence(timeout: 5))
    }

    private func selectTab(_ name: String) {
        if app.keyboards.firstMatch.exists {
            let dismiss = element("composer-dismiss-keyboard")
            XCTAssertTrue(dismiss.waitForExistence(timeout: 5), "The composer must provide an accessible keyboard dismissal action.")
            dismiss.tap()
            let hidden = XCTNSPredicateExpectation(predicate: NSPredicate(format: "exists == false"), object: app.keyboards.firstMatch)
            XCTAssertEqual(XCTWaiter.wait(for: [hidden], timeout: 5), .completed)
        }
        let tab = app.tabBars.buttons[name]
        XCTAssertTrue(tab.waitForExistence(timeout: 5))
        XCTAssertTrue(tab.isHittable, "The \(name) tab must be visible before navigation.")
        tab.tap()
        XCTAssertTrue(tab.isSelected)
    }

    private func send(_ content: String, usingReturn: Bool = false) {
        let field = element("composer-field")
        XCTAssertTrue(field.waitForExistence(timeout: 10))
        field.tap()
        field.typeText(content)
        let send = element("composer-send")
        XCTAssertTrue(send.isEnabled)
        if usingReturn { field.typeText("\n") }
        else { send.tap() }
    }

    private func contextAction(_ action: String, messageId: String) {
        let message = element("message-" + messageId)
        if !message.isHittable { app.swipeDown() }
        XCTAssertTrue(message.waitForExistence(timeout: 10))
        message.press(forDuration: 1.1)
        let button = app.buttons[action]
        XCTAssertTrue(button.waitForExistence(timeout: 5))
        button.tap()
    }

    private func element(_ identifier: String) -> XCUIElement {
        app.descendants(matching: .any).matching(identifier: identifier).firstMatch
    }

    private func screenshot(_ name: String) {
        let attachment = XCTAttachment(screenshot: app.screenshot())
        attachment.name = name
        attachment.lifetime = .keepAlways
        add(attachment)
    }

    private func waitForMessage(channel: String, content: String) async throws -> Message {
        for _ in 0..<50 {
            let messages: [Message] = try await request("/channels/\(channel)/messages")
            if let match = messages.first(where: { $0.content == content }) { return match }
            try await Task.sleep(for: .milliseconds(200))
        }
        XCTFail("The sent message was not persisted in the expected channel.")
        throw FixtureError.timedOut
    }

    private func waitForMessage(id: String, matching predicate: (Message) -> Bool) async throws -> Message {
        for _ in 0..<50 {
            let message: Message = try await request("/messages/\(id)")
            if predicate(message) { return message }
            try await Task.sleep(for: .milliseconds(200))
        }
        XCTFail("The server did not persist the requested message change.")
        throw FixtureError.timedOut
    }

    private func waitForDeletion(_ id: String) async throws {
        for _ in 0..<50 {
            let (_, status) = try await rawRequest("/messages/\(id)")
            if status == 404 { return }
            try await Task.sleep(for: .milliseconds(200))
        }
        XCTFail("The deleted message is still retrievable from the server.")
        throw FixtureError.timedOut
    }

    private func waitForAppearance(theme: String, mode: String?) async throws {
        for _ in 0..<50 {
            let appearance: Appearance = try await request("/users/me/appearance")
            if appearance.theme == theme && (mode == nil || appearance.mode == mode) { return }
            try await Task.sleep(for: .milliseconds(200))
        }
        XCTFail("The server did not save the selected appearance.")
        throw FixtureError.timedOut
    }

    private func postMessage(channel: String, content: String, user: Int) async throws -> Message {
        try await request("/channels/\(channel)/messages", method: "POST", body: ["content": content], user: user)
    }

    private func request<Value: Decodable>(_ path: String, method: String = "GET", body: [String: Any]? = nil, user: Int = 0) async throws -> Value {
        let (data, status) = try await rawRequest(path, method: method, body: body, user: user)
        guard (200..<300).contains(status) else {
            XCTFail("Isolated fixture API returned HTTP \(status) for \(method) \(path).")
            throw FixtureError.http(status)
        }
        return try JSONDecoder().decode(Value.self, from: data)
    }

    private func rawRequest(_ path: String, method: String = "GET", body: [String: Any]? = nil, user: Int = 0) async throws -> (Data, Int) {
        var request = URLRequest(url: try XCTUnwrap(URL(string: fixture.origin + path)))
        request.httpMethod = method
        request.setValue("Bearer " + fixture.users[user].session.token, forHTTPHeaderField: "Authorization")
        if let body {
            request.setValue("application/json", forHTTPHeaderField: "Content-Type")
            request.httpBody = try JSONSerialization.data(withJSONObject: body)
        }
        let (data, response) = try await URLSession.shared.data(for: request)
        return (data, try XCTUnwrap(response as? HTTPURLResponse).statusCode)
    }

    private enum FixtureError: Error { case invalidLocalFixture, timedOut, http(Int) }
    private struct Fixture: Decodable {
        let origin: String
        let generalChannelId: String
        let dmChannelId: String
        let users: [FixtureUser]
        enum CodingKeys: String, CodingKey {
            case origin, users
            case generalChannelId = "general_channel_id"
            case dmChannelId = "dm_channel_id"
        }
    }
    private struct FixtureUser: Decodable {
        let username: String
        let session: Session
        struct Session: Decodable {
            let token: String
            let user: User
            struct User: Decodable { let id: String }
        }
    }
    // The UI target cannot import the application. These narrow read-back projections deliberately
    // assert the wire response independently of the generated client used by the app itself.
    private struct Message: Decodable {
        let id: String
        let channelId: String
        let content: String
        let replyTo: String?
        let reactions: [Reaction]
        enum CodingKeys: String, CodingKey { case id, content, reactions; case channelId = "channel_id", replyTo = "reply_to" }
        struct Reaction: Decodable {
            let emoji: String
            let userIds: [String]
            enum CodingKeys: String, CodingKey { case emoji; case userIds = "user_ids" }
        }
    }
    private struct Appearance: Decodable { let theme: String; let mode: String }
}
