import UIKit
import XCTest

// A visual tour of Den Shell for Xcode Cloud's "Visual tour" workflow (`scripts/xcloud.sh tour`): every screen a
// person can reach, in light and in dark, saved as named screenshots for reviewers. It runs against the same made-up
// Den as the UI check (ci_scripts/ci_pre_xcodebuild.sh, ui-check/seed.py), as Ellis, plus Rowan for the admin pages.
// "Dark" is Den's own Dark setting: Xcode Cloud's iOS 27 simulator ignored `XCUIDevice.appearance` (2026-10-19), so
// iOS's own parts (keyboard, alerts, status bar) stay light in those shots. The device setting is still flipped, for a
// simulator that honours it. It only looks: nothing here asserts how a screen should look. A stop it can't reach
// keeps a `missing-…` screenshot and a `tree-missing-…` dump and the tour moves on to the next theme, so one run always
// says how far it got.
// Shots are named "<stop>-<light|dark>"; a long page scrolled through adds "-p2", "-p3"… to the stop.

final class TourUITests: XCTestCase {
    private var theme = "light"

    override func setUpWithError() throws {
        continueAfterFailure = true
    }

    /// Before signing in: the sign-in page, the invite page, a wrong password. Then the first sign-in and iOS's
    /// notification question, which only comes once (light only). The signed-out page takes the last account's look,
    /// so the dark half signs Ellis out with Den set to Dark.
    @MainActor func test01SignIn() async throws {
        await themes { theme in
            let app: XCUIApplication
            if theme == "light" {
                app = launch()
            } else {
                app = try launchSignedIn()
                try signOut(app)
            }
            let comeIn = try find(app, label: "Come in", timeout: 30)
            shot("sign-in")
            try find(app, label: "Have an invite?").tap()
            try find(app, label: "Join")
            shot("sign-up")
            try find(app, label: "Already a member? Log in").tap()
            fillSignIn(app, user: "ellis", password: "not my password")
            comeIn.tap()
            pause(2)
            shot("sign-in-wrong-password")
            // A fresh page for the real sign-in: typing over the wrong one left both in the field.
            let fresh = launch()
            let again = try find(fresh, label: "Come in", timeout: 30)
            fillSignIn(fresh, user: "ellis", password: password)
            again.tap()
            try find(fresh, label: "Your profile and settings", timeout: 20)
            let allow = springboard.alerts.buttons["Allow"]
            if allow.waitForExistence(timeout: theme == "light" ? 10 : 1) {
                shot("notification-question")
                allow.tap()
            }
        }
    }

    /// Home: rooms with unread counts and categories, the hangout card, the inbox badge.
    @MainActor func test02Home() async throws {
        await themes { _ in
            let app = try launchSignedIn()
            try find(app, label: "general", .prefix)
            shotScrolling("home", app)
        }
    }

    /// A busy room: the newest messages, then up through the history (pictures, a GIF, a link, reactions, the
    /// thread link, a long name, a bot).
    @MainActor func test03Room() async throws {
        await themes { _ in
            let app = try openGeneral()
            shotScrolling("room", app, up: false)
        }
    }

    /// The composer: keyboard up with a draft, then a photo waiting to send.
    @MainActor func test04Composer() async throws {
        await themes { _ in
            let app = try openGeneral()
            let composer = try find(app, label: "Message general")
            composer.tap()
            composer.typeText("on my way, ten minutes")
            _ = app.keyboards.firstMatch.waitForExistence(timeout: 5)
            shot("room-keyboard")
        }
        await themes { _ in
            let app = try openGeneral()
            try await attachPhoto(app)
            shot("room-photo-waiting")
        }
    }

    /// A long press on someone's message, the reaction picker, and the + sheet with its emoji and GIF sheets.
    @MainActor func test05Menus() async throws {
        await themes { _ in
            let app = try openGeneral()
            let message = try find(app, label: "Leaving 10am from my place", .contains)
            message.press(forDuration: 1)
            try find(app, label: "Copy text")
            shot("message-menu")
            try XCTUnwrap(nearest(app, label: "React to this message", above: message), "No reaction button by the message.").tap()
            pause()
            shot("reaction-picker")
        }
        await themes { _ in
            let app = try openGeneral()
            try find(app, label: "More").tap()
            try find(app, label: "Photo or file")
            shot("plus-sheet")
            try find(app, label: "Emoji").tap()
            pause()
            shot("emoji-sheet")
        }
        await themes { _ in
            let app = try openGeneral()
            try find(app, label: "More").tap()
            // The menu item reads "GIF GIF": its badge, then its name.
            try find(app, label: "GIF", .prefix).tap()
            pause()
            shot("gif-sheet")
        }
    }

    /// A thread, and replying to one message in it (the reply bar).
    @MainActor func test06Thread() async throws {
        await themes { _ in
            let app = try openGeneral()
            try find(app, label: "3 replies", .contains).tap()
            let reply = try find(app, label: "I need a ride, my car is in the shop")
            shot("thread")
            reply.press(forDuration: 1)
            try find(app, label: "Copy text")
            try XCTUnwrap(nearest(app, label: "Reply", above: reply), "No Reply button by the message.").tap()
            try find(app, label: "Cancel reply")
            putKeyboardAway(app, "thread-reply-bar")
            shot("thread-reply-bar")
        }
    }

    /// A DM, and a room nobody has written in yet.
    @MainActor func test07Conversations() async throws {
        await themes { _ in
            let app = try launchSignedIn()
            try find(app, label: "legend", .contains).tap()
            try find(app, label: "Message Theo", timeout: 10)
            shot("dm")
        }
        await themes { _ in
            let app = try launchSignedIn()
            try find(app, label: "books", .prefix).tap()
            try find(app, label: "Message books", timeout: 10)
            shot("empty-room")
        }
    }

    /// Search: focused on the home, its results, and the empty search page. Then the inbox.
    @MainActor func test08SearchInbox() async throws {
        await themes { _ in
            let app = try launchSignedIn()
            let field = try find(app, label: "Search messages")
            field.tap()
            pause()
            shot("home-search-focus")
            app.typeText("cabin\n")
            try find(app, label: "results", .contains, timeout: 10)
            pause()
            shot("search-results")
        }
        await themes { _ in
            let app = try openGeneral()
            let search = app.webViews.firstMatch.links.matching(NSPredicate(format: "label == 'Search messages'")).firstMatch
            try wait(for: search, "room's search link")
            search.tap()
            try find(app, label: "Words are matched exactly", .contains)
            shot("search")
        }
        await themes { _ in
            let app = try launchSignedIn()
            try find(app, label: "Inbox", .prefix).tap()
            try find(app, label: "Mark all read", .contains)
            shotScrolling("inbox", app)
        }
    }

    /// Someone's profile card, a bot's, and your own profile page.
    @MainActor func test09Profiles() async throws {
        await themes { _ in
            let app = try openGeneral()
            try find(app, label: "Maya's profile").tap()
            try find(app, label: "Message Maya")
            pause()
            shot("profile-maya")
        }
        await themes { _ in
            let app = try openGeneral()
            try find(app, label: "Pebble's profile").tap()
            pause()
            shot("profile-bot")
        }
        await themes { _ in
            let app = try launchSignedIn()
            try find(app, label: "Your profile and settings").tap()
            try find(app, label: "Ellis", .prefix).tap()
            pause()
            shotScrolling("profile-own", app)
        }
    }

    /// Settings: the list, then every page under it, scrolled through.
    @MainActor func test10Settings() async throws {
        await themes { _ in
            let app = try launchSignedIn()
            try find(app, label: "Your profile and settings").tap()
            try find(app, label: "Appearance")
            shotScrolling("settings", app)
            try settingsPages(app, ["Appearance", "Notifications", "Sounds", "Voice", "Spotify", "Layout", "Desktop app",
                                    "Account", "Machines", "Access", "API keys", "Agents"])
        }
    }

    /// The hangout. Xcode Cloud can't carry the call's media (see SmokeUITests.test5Call), so this gets as far as
    /// joining and the error that comes back. The in-call screen is shot on the web instead.
    @MainActor func test11Call() async throws {
        await themes { _ in
            let app = try launchSignedIn()
            let join = try find(app, label: "Join hangout")
            join.tap()
            let failed = element(app, label: "The call could not connect. Try again.")
            let joined = element(app, label: "Leave call")
            let allow = springboard.alerts.buttons["Allow"]
            var joining = false
            let deadline = Date().addingTimeInterval(45)
            while !failed.exists && !joined.exists && Date() < deadline {
                if allow.exists { shot("call-microphone-question"); allow.tap() }
                if !joining, join.exists, !join.isEnabled { joining = true; shot("call-joining") }
                _ = failed.waitForExistence(timeout: 1)
            }
            if joined.exists { shot("call-joined") } else if failed.exists { shot("call-error") } else {
                missing("call outcome", tree: app.debugDescription)
                throw Missing(description: "The join neither connected nor failed in 45 s.")
            }
        }
    }

    /// Den Shell pointed at a server that isn't answering.
    @MainActor func test12Unreachable() async throws {
        await themes { _ in
            let app = launch(url: "http://localhost:7999")
            pause(10)
            shot("server-unreachable")
            app.terminate()
        }
    }

    /// The pages only an admin sees, as Rowan. Signs Ellis out and back in around them.
    @MainActor func test13AdminSettings() async throws {
        let app = try launchSignedIn()
        try signOut(app)
        fillSignIn(app, user: "rowan", password: password)
        try find(app, label: "Come in").tap()
        try find(app, label: "Your profile and settings", timeout: 20)
        await themes(as: "rowan") { _ in
            let app = try launchSignedIn(as: "rowan")
            try find(app, label: "Your profile and settings").tap()
            try find(app, label: "Members")
            shotScrolling("settings-admin", app)
            try settingsPages(app, ["Members", "Invites", "Rooms", "Plugins"])
        }
        try signOut(try launchSignedIn(as: "rowan"))
        _ = try launchSignedIn()
    }

    // MARK: Helpers

    private let password = "correct horse battery"

    /// Runs `body` once in light and once in dark, each on a fresh launch: Den's own setting for `user`, and the
    /// simulator's. A stop that fails is recorded and the next theme still runs. Den goes back to following the system.
    @MainActor private func themes(as user: String = "ellis", _ body: (String) async throws -> Void) async {
        for (name, look) in [("light", XCUIDevice.Appearance.light), ("dark", .dark)] {
            theme = name
            XCUIDevice.shared.appearance = look
            do {
                try await setMode(name, for: user)
                try await body(name)
            } catch { XCTFail("\(name): \(error)") }
        }
        theme = "light"
        XCUIDevice.shared.appearance = .light
        do { try await setMode("system", for: user) } catch { XCTFail("system: \(error)") }
    }

    /// Den's "Light or dark" setting, over the API as the page saves it. An open page follows at once.
    @MainActor private func setMode(_ mode: String, for user: String) async throws {
        let token = try await token(user)
        guard var appearance = try await api("GET", "/users/me/appearance", token: token) as? [String: Any] else {
            throw Missing(description: "No appearance for \(user).")
        }
        appearance["mode"] = mode
        _ = try await api("PUT", "/users/me/appearance", appearance, token: token)
    }

    /// One API sign-in per person for the whole run: Den allows 10 sign-ins a minute, the app's own included.
    @MainActor private static var tokens: [String: String] = [:]

    @MainActor private func token(_ user: String) async throws -> String {
        if let token = Self.tokens[user] { return token }
        let token = try await signIn(user)
        Self.tokens[user] = token
        return token
    }

    /// Well inside the web view, for things the page draws. XCUITest's own hit test (isHittable, tap) failed outright
    /// on a Settings link 2 pt from the bottom edge, and that ends the whole test, so these get tapped by coordinate.
    /// 44 pt, not more: the list's last row (Agents) can't scroll further than 80 pt from the edge.
    @MainActor private func onScreen(_ element: XCUIElement, _ app: XCUIApplication) -> Bool {
        element.exists && app.webViews.firstMatch.frame.insetBy(dx: 0, dy: 44)
            .contains(CGPoint(x: element.frame.midX, y: element.frame.midY))
    }

    /// Scrolls the page towards `element` until it is well inside the view, then taps its centre.
    @MainActor private func scrollAndTap(_ element: XCUIElement, _ name: String, _ app: XCUIApplication) throws {
        let web = app.webViews.firstMatch
        for _ in 0..<8 where !onScreen(element, app) {
            if element.frame.midY < web.frame.midY { web.swipeDown(velocity: .slow) } else { web.swipeUp(velocity: .slow) }
            pause(0.5)
        }
        guard onScreen(element, app) else {
            missing(name, tree: app.debugDescription)
            throw Missing(description: "Couldn't scroll \(name) into view.")
        }
        element.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.5)).tap()
    }

    @MainActor private func launch(url: String = "http://localhost:7000") -> XCUIApplication {
        let app = XCUIApplication()
        app.launchArguments = ["-DenStartURL", url, "-DenCallTestTone", "YES",
                               "-DenTestPushToken", String(repeating: "0d", count: 32)]
        app.launch()
        return app
    }

    /// Opens Den Shell signed in, signing in again if the web view lost the session.
    @MainActor private func launchSignedIn(as user: String = "ellis") throws -> XCUIApplication {
        let app = launch()
        let rooms = element(app, label: "Your profile and settings")
        let comeIn = element(app, label: "Come in")
        let deadline = Date().addingTimeInterval(30)
        while !rooms.exists && !comeIn.exists && Date() < deadline { _ = rooms.waitForExistence(timeout: 1) }
        if comeIn.exists {
            fillSignIn(app, user: user, password: password)
            comeIn.tap()
        }
        try find(app, label: "Your profile and settings", timeout: 20)
        let allow = springboard.alerts.buttons["Allow"]
        if allow.waitForExistence(timeout: 1) { allow.tap() }
        return app
    }

    @MainActor private func openGeneral() throws -> XCUIApplication {
        let app = try launchSignedIn()
        try find(app, label: "general", .prefix).tap()
        try find(app, label: "Message general")
        pause()
        return app
    }

    @MainActor private func fillSignIn(_ app: XCUIApplication, user: String, password: String) {
        let web = app.webViews.firstMatch
        let name = web.textFields.firstMatch, secret = web.secureTextFields.firstMatch
        name.tap()
        name.clear()
        name.typeText(user)
        secret.tap()
        secret.clear()
        secret.typeText(password)
    }

    @MainActor private func signOut(_ app: XCUIApplication) throws {
        try find(app, label: "Your profile and settings").tap()
        try find(app, label: "Account").tap()
        try scrollAndTap(try find(app, label: "Log out"), "Log out", app)
        try find(app, label: "Come in", timeout: 15)
    }

    /// Each Settings page in turn, scrolled through, then back to the list. A page this account doesn't have is skipped.
    @MainActor private func settingsPages(_ app: XCUIApplication, _ pages: [String]) throws {
        for page in pages {
            let row = app.webViews.firstMatch.links.matching(NSPredicate(format: "label == %@", page)).firstMatch
            guard row.exists else { continue }
            try scrollAndTap(row, page, app)
            pause()
            shotScrolling("settings-" + page.lowercased().replacingOccurrences(of: " ", with: "-"), app)
            let back = app.webViews.firstMatch.buttons.matching(NSPredicate(format: "label == 'Settings'")).firstMatch
            try wait(for: back, "Settings back button")
            back.tap()
            try find(app, label: "Appearance")
        }
    }

    /// A photo waiting in the composer, the way a person adds one: + → Photo or file → Photo Library → pick one.
    /// If the simulator's library has nothing to pick, pastes Maya's lake picture from the made-up Den instead.
    @MainActor private func attachPhoto(_ app: XCUIApplication) async throws {
        try find(app, label: "More").tap()
        try find(app, label: "Photo or file").tap()
        let library = app.descendants(matching: .any).matching(NSPredicate(format: "label == 'Photo Library'")).firstMatch
        if library.waitForExistence(timeout: 5) {
            shot("file-menu")
            library.tap()
            pause(3)
            shot("photo-picker")
            // The picker's cells, not the page's pictures behind it.
            let photo = app.images.matching(NSPredicate(format: "label BEGINSWITH 'Photo'")).firstMatch
            if photo.waitForExistence(timeout: 5) {
                photo.tap()
                let add = app.buttons["Add"]
                if add.waitForExistence(timeout: 3) { add.tap() }
                if element(app, label: "Remove").waitForExistence(timeout: 10) { return putKeyboardAway(app, "room-photo-waiting") }
            }
            let cancel = app.buttons["Cancel"]
            if cancel.exists { cancel.tap() }
        }
        let token = try await token("maya")
        let plans = try await roomId("plans", token: token)
        let messages = try await api("GET", "/channels/\(plans)/messages?limit=20", token: token) as? [[String: Any]] ?? []
        let upload = try XCTUnwrap(messages.compactMap { ($0["attachments"] as? [[String: Any]])?.first?["id"] as? String }.first)
        let picture = try await data("/uploads/\(upload)/file", token: token)
        UIPasteboard.general.image = try XCTUnwrap(UIImage(data: picture))
        let composer = try find(app, label: "Message general")
        composer.tap()
        composer.press(forDuration: 1.2)
        let paste = app.descendants(matching: .any).matching(NSPredicate(format: "label == 'Paste'")).firstMatch
        try wait(for: paste, "Paste")
        paste.tap()
        try find(app, label: "Remove", timeout: 10)
        putKeyboardAway(app, "room-photo-waiting")
    }

    /// The composer keeps focus after a photo is added or a reply starts, and the keyboard covers what changed:
    /// shoot that as "<stop>-keyboard", then lower it.
    @MainActor private func putKeyboardAway(_ app: XCUIApplication, _ stop: String) {
        pause()
        let tip = app.buttons["Continue"]
        if tip.exists { shot("keyboard-tip"); tip.tap() }
        shot("\(stop)-keyboard")
        let done = app.buttons["Done"]
        if done.exists { done.tap() } else { app.webViews.firstMatch.swipeDown(velocity: .slow) }
        pause()
    }

    /// The page's element labelled `label` closest above `target`: the tool bar a long press shows sits just above
    /// its message, while every other message keeps an invisible one of its own.
    @MainActor private func nearest(_ app: XCUIApplication, label: String, above target: XCUIElement) -> XCUIElement? {
        let top = target.frame.minY
        return app.webViews.firstMatch.descendants(matching: .any).matching(NSPredicate(format: "label == %@", label))
            .allElementsBoundByIndex.filter { $0.frame.maxY <= top + 8 }.min { top - $0.frame.maxY < top - $1.frame.maxY }
    }

    @MainActor private var springboard: XCUIApplication { XCUIApplication(bundleIdentifier: "com.apple.springboard") }

    @MainActor private func pause(_ seconds: TimeInterval = 1) {
        _ = XCTWaiter.wait(for: [XCTestExpectation(description: "settle")], timeout: seconds)
    }

    @MainActor private func shot(_ stop: String) {
        let attachment = XCTAttachment(screenshot: XCUIScreen.main.screenshot())
        attachment.name = "\(stop)-\(theme)"
        attachment.lifetime = .keepAlways
        add(attachment)
    }

    /// Shoots a page, then swipes through it (`up`: towards the end) shooting each screenful, until it stops moving.
    @MainActor private func shotScrolling(_ stop: String, _ app: XCUIApplication, up: Bool = true, pages: Int = 6) {
        pause()
        shot(stop)
        let web = app.webViews.firstMatch
        var last = content(web)
        for part in 2...pages {
            if up { web.swipeUp(velocity: .slow) } else { web.swipeDown(velocity: .slow) }
            pause(1.5)
            let now = content(web)
            if now == last { break }
            last = now
            shot("\(stop)-p\(part)")
        }
    }

    /// The page as drawn, minus the right edge where the scroll bar flashes after every swipe, even one that didn't move.
    @MainActor private func content(_ web: XCUIElement) -> Data? {
        let image = web.screenshot().image
        guard let cg = image.cgImage else { return nil }
        let edge = Int(16 * image.scale)
        return cg.cropping(to: CGRect(x: 0, y: 0, width: cg.width - edge, height: cg.height)).flatMap { UIImage(cgImage: $0).pngData() }
    }

    // The rest matches SmokeUITests: the page's elements by accessible label, and Maya's side over the API.

    @MainActor private func element(_ app: XCUIApplication, label: String, _ match: Match = .exact) -> XCUIElement {
        let predicate = NSPredicate(format: "label \(match.rawValue) %@", label)
        return app.webViews.firstMatch.descendants(matching: .any).matching(predicate).firstMatch
    }

    @MainActor @discardableResult
    private func find(_ app: XCUIApplication, label: String, _ match: Match = .exact, timeout: TimeInterval = 10) throws -> XCUIElement {
        let found = element(app, label: label, match)
        if found.waitForExistence(timeout: timeout) { return found }
        missing(label, tree: app.debugDescription)
        throw Missing(description: "Nothing labelled \"\(label)\" (\(match)) on screen after \(Int(timeout)) s.")
    }

    @MainActor private func wait(for element: XCUIElement, _ name: String, timeout: TimeInterval = 10) throws {
        if element.waitForExistence(timeout: timeout) { return }
        missing(name, tree: XCUIApplication().debugDescription + "\n\n" + springboard.debugDescription)
        throw Missing(description: "No \(name) after \(Int(timeout)) s.")
    }

    @MainActor private func missing(_ name: String, tree: String) {
        shot("missing-\(name)")
        let attachment = XCTAttachment(string: tree)
        attachment.name = "tree-missing-\(name)-\(theme)"
        attachment.lifetime = .keepAlways
        add(attachment)
    }

    @MainActor private func signIn(_ username: String) async throws -> String {
        let session = try await api("POST", "/auth/login", ["username": username, "password": password], token: nil)
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
        let reply = try await data(path, method: method, body: body, token: token)
        return try JSONSerialization.jsonObject(with: reply)
    }

    @MainActor private func data(_ path: String, method: String = "GET", body: [String: Any]? = nil, token: String?) async throws -> Data {
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
        return data
    }

    enum Match: String { case exact = "==", prefix = "BEGINSWITH", contains = "CONTAINS" }

    struct Missing: Error, CustomStringConvertible { let description: String }
}

private extension XCUIElement {
    /// Empties a text field the page may have kept from an earlier try.
    @MainActor func clear() {
        guard let text = value as? String, !text.isEmpty, text != placeholderValue else { return }
        coordinate(withNormalizedOffset: CGVector(dx: 0.95, dy: 0.5)).tap()
        typeText(String(repeating: XCUIKeyboardKey.delete.rawValue, count: text.count))
    }
}
