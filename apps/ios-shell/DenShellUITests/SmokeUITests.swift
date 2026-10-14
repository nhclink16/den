import XCTest

// A smoke check of Den Shell for Xcode Cloud's "UI check" workflow. It runs against a made-up Den on the runner
// itself (ci_scripts/ci_pre_xcodebuild.sh starts it from the branch's build), signs in as Ellis and keeps a named
// screenshot of each screen in the result bundle. `scripts/xcloud.sh ui` brings them back as PNG files.
// Few, stable flows: a failure here should mean Den Shell or the web app broke, not that the test is fussy.

final class SmokeUITests: XCTestCase {
    override func setUpWithError() throws {
        continueAfterFailure = false
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
        try find(app, label: "Photo or file")
        shot("3-plus-sheet")
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
        // iOS may ask for the microphone while joining.
        let joined = element(app, label: "You're in", .contains)
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
            try find(app, label: "Dismiss").tap()
            XCTAssertTrue(try find(app, label: "Join hangout", timeout: 5).isEnabled, "Join didn't come back.")
            shot("7-call-after-error")
            return
        }
        try find(app, label: "You're in", .contains, timeout: 1)
        shot("6-call-joined")
        try find(app, label: "Open hangout").tap()
        try find(app, label: "Leave call")
        shot("7-call-view")
        try find(app, label: "Leave call").tap()
        try find(app, label: "Join hangout", timeout: 15)
        shot("8-call-left")
    }

    // MARK: Helpers

    /// Opens Den Shell on the runner's Den, signing in as Ellis unless the web view still has the session.
    @MainActor private func launchSignedIn() throws -> XCUIApplication {
        let app = XCUIApplication()
        app.launchArguments = ["-DenStartURL", "http://localhost:7000", "-DenCallTestTone", "YES"]
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
        return app
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
