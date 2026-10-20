import XCTest

// A smoke check of Den Shell for Xcode Cloud's "UI check" workflow. It runs against a made-up Den on the runner
// itself (ci_scripts/ci_pre_xcodebuild.sh starts it from the branch's build), signs in as Ellis and keeps a named
// screenshot of each screen in the result bundle. `scripts/xcloud.sh ui` brings them back as PNG files.
// Few, stable flows: a failure here should mean Den Shell or the web app broke, not that the test is fussy.

final class SmokeUITests: XCTestCase {
    override func setUpWithError() throws {
        continueAfterFailure = false
    }

    /// Runs first, so it sees Den's question and then iOS's one-time prompt. The page registers this iPhone with the
    /// made-up Den once Ellis says yes;
    /// Settings only says "On" once Den took the token. Then Maya mentions Ellis over the API, a stand-in for Apple's
    /// push service on the runner (ci_scripts/ci_pre_xcodebuild.sh) delivers Den's alert for it, and tapping the
    /// banner must open that room: once with Den in the background, once with Den closed.
    @MainActor func test0Notifications() async throws {
        let app = try launchSignedIn(notificationPrompt: true)
        try find(app, label: "Your profile and settings").tap()
        try find(app, label: "Notifications").tap()
        try find(app, label: "On. ", .prefix, timeout: 15)
        shot("0-notifications-on")

        XCUIDevice.shared.press(.home)
        try await mention("@ellis push check: can you grab ice on the way?", in: "plans")
        try tapBanner(containing: "grab ice", shot: "0-banner-background", app: app)
        try find(app, label: "Message plans", timeout: 15)
        try find(app, label: "grab ice", .contains)
        shot("0-opened-from-background")

        app.terminate()
        try await mention("@ellis push check: the speaker is in my car", in: "music")
        try tapBanner(containing: "speaker is in my car", shot: "0-banner-closed", app: app)
        try find(app, label: "Message music", timeout: 30)
        try find(app, label: "speaker is in my car", .contains)
        shot("0-opened-from-closed")
    }

    /// A reply typed into Den's notification with Den closed: iOS starts Den Shell in the background with no page, and
    /// the shell sends the reply itself with the page's sign-in. Maya must then see it in the room, from Ellis, and Den
    /// must stay in the background.
    @MainActor func test0QuickReply() async throws {
        let app = try launchSignedIn()
        app.terminate()
        try await mention("@ellis push check: who has the spare key?", in: "minecraft")
        let banner = try notification(containing: "spare key")
        shot("0-reply-banner")
        banner.press(forDuration: 1.5)
        // iOS 27 opens the reply field straight away when the only action takes text; older iOS shows Reply first.
        let field = springboard.textViews.matching(NSPredicate(format: "placeholderValue == 'Reply'")).firstMatch
        if !field.waitForExistence(timeout: 3), springboard.buttons["Reply"].exists { springboard.buttons["Reply"].tap() }
        try wait(for: field, "reply field")
        field.typeText("I do, by the door")
        let send = springboard.buttons["Send"]
        try wait(for: send, "Send button")
        shot("0-reply-typed")
        send.tap()
        let maya = try await signIn("maya")
        let users = try await api("GET", "/users", token: maya) as? [[String: Any]] ?? []
        let ellis = try XCTUnwrap(users.first { $0["username"] as? String == "ellis" }?["id"] as? String)
        let room = try await roomId("minecraft", token: maya)
        let deadline = Date().addingTimeInterval(30)
        while Date() < deadline {
            let messages = try await api("GET", "/channels/\(room)/messages?limit=5", token: maya) as? [[String: Any]] ?? []
            if messages.contains(where: { $0["content"] as? String == "I do, by the door" && $0["author_id"] as? String == ellis }) {
                shot("0-reply-sent")
                XCTAssertNotEqual(app.state, .runningForeground, "Replying opened Den.")
                return
            }
            try await Task.sleep(for: .seconds(1))
        }
        shot("missing-reply")
        XCTFail("Ellis's reply never reached the room.")
    }

    @MainActor func test1RoomList() throws {
        let app = try launchSignedIn()
        try find(app, label: "general", .prefix)
        shot("1-room-list")
    }

    @MainActor func test2RoomWithKeyboard() throws {
        let app = try launchSignedIn()
        try find(app, label: "general", .prefix).tap()
        let composer = try find(app, label: "Message general")
        composer.tap()
        composer.typeText("on my way")
        XCTAssertTrue(app.keyboards.firstMatch.waitForExistence(timeout: 5), "The keyboard didn't come up.")
        shot("2-room-keyboard")
    }

    @MainActor func test3PlusSheet() throws {
        let app = try launchSignedIn()
        try find(app, label: "general", .prefix).tap()
        try find(app, label: "More").tap()
        try find(app, label: "Photos")
        shot("3-plus-sheet")
    }

    /// The home's search field opens Search with its field focused and the keyboard up; results come back.
    @MainActor func test6Search() throws {
        let app = try launchSignedIn()
        try find(app, label: "Search messages").tap()
        XCTAssertTrue(app.keyboards.firstMatch.waitForExistence(timeout: 5), "The keyboard didn't come up in Search.")
        try find(app, label: "Cancel")
        shot("9-search-open")
        app.typeText("cabin\n")
        try find(app, label: "results", .contains, timeout: 10)
        shot("9-search-results")
    }

    @MainActor func test4Settings() throws {
        let app = try launchSignedIn()
        try find(app, label: "Your profile and settings").tap()
        try find(app, label: "Appearance")
        shot("4-settings")
    }

    /// The call runs in the native engine (debug simulator builds skip CallKit, which the simulator can't hold).
    /// `-DenCallTestTone` swaps the mic's sound for a test hum. On Xcode Cloud the call's media can't connect: the
    /// simulator's only address (10.255.0.2) refuses connections, even from the same machine. So the join reaches
    /// LiveKit and then times out, and this checks that the native engine's error comes back to the page and Den
    /// recovers. Where media does connect, it checks joining, the call view and Leave instead.
    @MainActor func test5Call() throws {
        let app = try launchSignedIn()
        let join = try find(app, label: "Join hangout")
        join.tap()
        // iOS may ask for the microphone while joining. Once joined, the home shows the call pill (with
        // Leave) instead of the hangout card.
        let joined = element(app, label: "Leave call")
        let failed = element(app, label: "The call could not connect. Try again.")
        let allow = XCUIApplication(bundleIdentifier: "com.apple.springboard").alerts.buttons["Allow"]
        let deadline = Date().addingTimeInterval(45)
        var joining = false
        while !joined.exists && !failed.exists && Date() < deadline {
            if allow.exists { allow.tap() }
            // The button says Joining… and is disabled until the native engine answers.
            if !joining, join.exists, !join.isEnabled { joining = true; shot("5-call-joining") }
            _ = joined.waitForExistence(timeout: 1)
        }
        if failed.exists {
            XCTAssertTrue(joining, "Join was never disabled while joining.")
            shot("6-call-no-media")
            // The error is a toast that also goes by itself; a tap puts it away now.
            failed.tap()
            // The hangout row offers Retry after a failed join.
            XCTAssertTrue(try find(app, label: "Retry joining hangout", timeout: 5).isEnabled, "Retry didn't come.")
            shot("7-call-after-error")
            return
        }
        try find(app, label: "Leave call", timeout: 1)
        shot("6-call-joined")
        try find(app, label: "open the call", .contains).tap()
        try find(app, label: "Collapse call")
        shot("7-call-view")
        try find(app, label: "Leave call").tap()
        try find(app, label: "Join hangout", timeout: 15)
        shot("8-call-left")
    }

    /// Touch: the + sheet closes when pulled down, and a sent message asks for a haptic (the simulator has none to
    /// play, so the proof is "haptic light" in Den Shell's log). Each is checked on its own, so one miss shows the other.
    /// Not here: swipe back from the left edge. XCUITest's edge drag didn't start WebKit's back gesture (2026-10-19,
    /// nor did AXe's), so it's checked on a phone.
    @MainActor func test6Touch() throws {
        continueAfterFailure = true
        let app = try launchSignedIn()
        try find(app, label: "general", .prefix).tap()
        try find(app, label: "More").tap()
        let photo = try find(app, label: "Photos")
        let grip = photo.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.5))
        grip.press(forDuration: 0.05, thenDragTo: grip.withOffset(CGVector(dx: 0, dy: 320)))
        XCTAssertTrue(photo.waitForNonExistence(timeout: 3), "Pulling the + sheet down didn't close it.")
        shot("6-sheet-pulled-closed")

        let composer = try find(app, label: "Message general")
        composer.tap()
        composer.typeText("haptic check")
        try find(app, label: "Send").tap()
        XCTAssertTrue(element(app, label: "haptic check", .contains).waitForExistence(timeout: 5), "The message didn't send.")
        shot("6-sent")
    }

    // MARK: Helpers

    /// Opens Den Shell on the runner's Den, signing in as Ellis unless the web view still has the session.
    /// `-DenTestPushToken`: the simulator may get no token from Apple here, so it registers a made-up one.
    @MainActor private func launchSignedIn(notificationPrompt: Bool = false) throws -> XCUIApplication {
        let app = XCUIApplication()
        app.launchArguments = ["-DenStartURL", "http://localhost:7000", "-DenCallTestTone", "YES",
                               "-DenTestPushToken", String(repeating: "0d", count: 32)]
        app.launch()
        let rooms = element(app, label: "Your profile and settings")
        let comeIn = element(app, label: "Come in")
        let deadline = Date().addingTimeInterval(30)
        while !rooms.exists && !comeIn.exists && Date() < deadline { _ = rooms.waitForExistence(timeout: 1) }
        if comeIn.exists {
            let web = app.webViews.firstMatch
            web.textFields.firstMatch.tap()
            web.textFields.firstMatch.typeText("ellis")
            web.secureTextFields.firstMatch.tap()
            web.secureTextFields.firstMatch.typeText("correct horse battery")
            shot("0-sign-in")
            comeIn.tap()
        }
        try find(app, label: "Your profile and settings", timeout: 20)
        // After the first sign-in Den asks about notifications in its own words; Turn On brings up iOS's one-time prompt.
        let turnOn = element(app, label: "Turn On")
        guard turnOn.waitForExistence(timeout: notificationPrompt ? 10 : 2) else {
            if notificationPrompt { XCTFail("Den didn't ask about notifications after sign-in.") }
            return app
        }
        if notificationPrompt { shot("0-notification-question") }
        turnOn.tap()
        let allow = springboard.alerts.buttons["Allow"]
        if allow.waitForExistence(timeout: 10) {
            if notificationPrompt { shot("0-notification-prompt") }
            allow.tap()
        } else if notificationPrompt {
            XCTFail("iOS didn't ask about notifications after Turn On.")
        }
        return app
    }

    @MainActor private var springboard: XCUIApplication { XCUIApplication(bundleIdentifier: "com.apple.springboard") }

    /// Waits for Den's notification on the Home Screen and taps it; iOS must then bring Den to the front.
    @MainActor private func tapBanner(containing text: String, shot name: String, app: XCUIApplication) throws {
        let notification = try notification(containing: text)
        shot(name)
        notification.tap()
        guard app.wait(for: .runningForeground, timeout: 10) else {
            shot("\(name)-not-opened")
            throw Missing(description: "Tapping the notification didn't bring Den to the front.")
        }
    }

    @MainActor private func notification(containing text: String) throws -> XCUIElement {
        let notification = springboard.descendants(matching: .any).matching(NSPredicate(format: "label CONTAINS %@", text)).firstMatch
        try wait(for: notification, "banner", timeout: 30)
        return notification
    }

    /// For iOS's own screens. On a miss, keeps a screenshot and what the Home Screen showed.
    @MainActor private func wait(for element: XCUIElement, _ name: String, timeout: TimeInterval = 10) throws {
        if element.waitForExistence(timeout: timeout) { return }
        shot("missing-\(name)")
        let tree = XCTAttachment(string: springboard.debugDescription)
        tree.name = "tree-missing-\(name)"
        tree.lifetime = .keepAlways
        add(tree)
        throw Missing(description: "No \(name) after \(Int(timeout)) s.")
    }

    /// Maya posts in a room over the API, the way her own phone would.
    @MainActor private func mention(_ text: String, in room: String) async throws {
        let token = try await signIn("maya")
        let id = try await roomId(room, token: token)
        _ = try await api("POST", "/channels/\(id)/messages", ["content": text], token: token)
    }

    @MainActor private func signIn(_ username: String) async throws -> String {
        let session = try await api("POST", "/auth/login", ["username": username, "password": "correct horse battery"], token: nil)
        guard let token = (session as? [String: Any])?["token"] as? String else { throw Missing(description: "No token for \(username).") }
        return token
    }

    @MainActor private func roomId(_ room: String, token: String) async throws -> String {
        let channels = try await api("GET", "/channels", token: token) as? [[String: Any]] ?? []
        guard let id = channels.first(where: { $0["name"] as? String == room })?["id"] as? String else {
            throw Missing(description: "No room called \(room).")
        }
        return id
    }

    @MainActor private func api(_ method: String, _ path: String, _ body: [String: Any]? = nil, token: String?) async throws -> Any {
        var request = URLRequest(url: URL(string: "http://localhost:7000\(path)")!)
        request.httpMethod = method
        request.setValue("application/json", forHTTPHeaderField: "content-type")
        request.setValue("http://localhost:7000", forHTTPHeaderField: "origin")
        if let token { request.setValue("Bearer \(token)", forHTTPHeaderField: "authorization") }
        if let body { request.httpBody = try JSONSerialization.data(withJSONObject: body) }
        let (data, response) = try await URLSession.shared.data(for: request)
        guard (response as? HTTPURLResponse)?.statusCode == 200 else {
            throw Missing(description: "\(method) \(path) answered \(String(data: data, encoding: .utf8) ?? "")")
        }
        return try JSONSerialization.jsonObject(with: data)
    }

    enum Match: String { case exact = "==", prefix = "BEGINSWITH", contains = "CONTAINS" }

    @MainActor private func element(_ app: XCUIApplication, label: String, _ match: Match = .exact) -> XCUIElement {
        let predicate = NSPredicate(format: "label \(match.rawValue) %@", label)
        return app.webViews.firstMatch.descendants(matching: .any).matching(predicate).firstMatch
    }

    /// Waits for an element of the page by its accessible label. On a miss, keeps a screenshot and the
    /// accessibility tree, so one failed cloud run says what the screen showed instead.
    @MainActor @discardableResult
    private func find(_ app: XCUIApplication, label: String, _ match: Match = .exact, timeout: TimeInterval = 10) throws -> XCUIElement {
        let found = element(app, label: label, match)
        if found.waitForExistence(timeout: timeout) { return found }
        shot("missing-\(label)")
        let tree = XCTAttachment(string: app.debugDescription)
        tree.name = "tree-missing-\(label)"
        tree.lifetime = .keepAlways
        add(tree)
        throw Missing(description: "Nothing labelled \"\(label)\" (\(match)) on screen after \(Int(timeout)) s.")
    }

    struct Missing: Error, CustomStringConvertible { let description: String }

    @MainActor private func shot(_ name: String) {
        let attachment = XCTAttachment(screenshot: XCUIScreen.main.screenshot())
        attachment.name = name
        attachment.lifetime = .keepAlways
        add(attachment)
    }
}
