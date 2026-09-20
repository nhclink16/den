import Foundation
import UIKit
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
        if ProcessInfo.processInfo.environment["DEN_UI_ACCESSIBILITY_TEXT"] == "1" {
            app.launchArguments += ["-UIPreferredContentSizeCategoryName",
                                    UIContentSizeCategory.accessibilityExtraExtraExtraLarge.rawValue]
        }
        app.launchEnvironment["DEN_UI_FIXTURE_PATH"] = fixturePath
        app.launchEnvironment["DEN_UI_USER"] = "0"
        addUIInterruptionMonitor(withDescription: "Den notification permission") { alert in
            let text = alert.staticTexts.allElementsBoundByIndex.map(\.label).joined(separator: " ").lowercased()
            guard text.contains("notification"), alert.buttons["Allow"].exists else { return false }
            alert.buttons["Allow"].tap()
            return true
        }
    }

    func testFirstRunDictationPermissionsAndFirstPCMFrame() async throws {
        try configureFixture()
        try consumeDictationPrivacyReset()
        app.launchArguments.append("--den-ui-dictation-pcm")
        let before: [Message] = try await request("/channels/\(fixture.generalChannelId)/messages")
        let draft = "Keep this draft"
        let expected = draft + " local dictation sample"
        let system = XCUIApplication(bundleIdentifier: "com.apple.springboard")
        // iOS 27 exposes the speech sheet as Other, not Alert. Match its visible
        // system title and quoted app name independently of the container role.
        let microphone = system.staticTexts.matching(NSPredicate(
            format: "(label CONTAINS %@ OR label CONTAINS %@) AND label CONTAINS[c] %@",
            "“Den”", "\"Den\"", "microphone")).firstMatch
        let speech = system.staticTexts.matching(NSPredicate(
            format: "(label CONTAINS %@ OR label CONTAINS %@) AND label CONTAINS[c] %@",
            "“Den”", "\"Den\"", "speech recognition")).firstMatch
        defer {
            // Failure must not leave our permission sheet live across the next reset.
            // Never dismiss any system UI without the matching visible Den title.
            for title in [microphone, speech] where title.exists {
                let deny = permissionButton(system: system, title: title, labels: ["Don’t Allow", "Don't Allow"])
                if deny.exists && deny.isHittable { deny.tap() }
            }
            app.terminate()
        }

        app.launch()
        XCTAssertTrue(element("login-submit").waitForExistence(timeout: 10))
        element("login-submit").tap()
        try await waitForRooms()
        openRoom(fixture.generalChannelId)
        let field = element("composer-field")
        field.tap()
        field.typeText(draft)
        XCTAssertEqual(field.value as? String, draft)
        let dictate = element("composer-dictate")
        XCTAssertTrue(dictate.waitForExistence(timeout: 10))
        XCTAssertTrue(dictate.isHittable)
        XCTAssertEqual(dictate.label, "Dictate")
        dictate.tap()

        // This dialog is part of the tested flow, not an interruption to dismiss implicitly.
        guard microphone.waitForExistence(timeout: 10) else {
            XCTFail("Fresh privacy state must show the real microphone prompt.")
            return
        }
        guard !speech.exists else {
            XCTFail("Local dictation must not request Apple's speech service permission.")
            return
        }
        // Underlying app AX is stale while this modal is visible. Do not query it.
        // Ownership is tested after dismissal by processing PCM from this same tap.
        let permissionShot = XCTAttachment(screenshot: system.screenshot())
        permissionShot.name = "dictation-first-run-microphone-permission"
        permissionShot.lifetime = .keepAlways
        add(permissionShot)
        let allow = permissionButton(system: system, title: microphone, labels: ["Allow", "OK"])
        guard microphone.exists, !speech.exists, allow.exists, allow.isHittable else {
            XCTFail("The verified microphone title must still be visible with a hittable affirmative action and no speech prompt.")
            return
        }
        allow.tap()
        let dismissed = XCTNSPredicateExpectation(predicate: NSPredicate(format: "exists == false"), object: microphone)
        guard await XCTWaiter.fulfillment(of: [dismissed], timeout: 5) == .completed else {
            XCTFail("The microphone prompt must disappear after Allow.")
            return
        }
        guard !speech.waitForExistence(timeout: 2) else {
            XCTFail("Local dictation must not show a second speech permission title.")
            return
        }

        // The Debug source only advances recordingDuration after writing actual PCM
        // through the production mono file writer. No text is committed during capture.
        let recording = element("dictation-recording")
        XCTAssertTrue(recording.waitForExistence(timeout: 10))
        for _ in 0..<100 {
            guard !speech.exists, !microphone.exists, app.state != .notRunning else {
                XCTFail("The original microphone tap must survive permission and first PCM processing.")
                return
            }
            let duration = Double((recording.value as? String ?? "").split(separator: " ").first.map(String.init) ?? "") ?? 0
            if duration > 0 { break }
            try await Task.sleep(for: .milliseconds(100))
        }
        let duration = Double((recording.value as? String ?? "").split(separator: " ").first.map(String.init) ?? "") ?? 0
        XCTAssertGreaterThan(duration, 0, "Recording must process PCM after the first microphone prompt, without another tap.")
        XCTAssertEqual(field.value as? String, draft, "Recording must leave the original draft untouched.")
        XCTAssertEqual(app.state, .runningForeground)
        XCTAssertFalse(element("composer-dismiss-keyboard").exists, "No accessory Done row may consume keyboard space.")
        XCTAssertFalse(dictate.exists, "The mic must be replaced by recording controls, not a pulsing icon.")
        XCTAssertTrue(element("composer-dictation-cancel").isHittable)
        let finish = element("composer-dictation-finish")
        XCTAssertTrue(finish.isHittable)
        XCTAssertTrue(app.keyboards.firstMatch.exists, "Starting dictation must preserve the existing keyboard.")
        field.typeText(" must not be inserted")
        XCTAssertEqual(field.value as? String, draft, "The visible draft is read-only while recording.")
        XCTAssertTrue(recording.exists)
        screenshot("dictation-first-run-recording")

        finish.tap()
        let completed = XCTNSPredicateExpectation(predicate: NSPredicate(format: "value == %@", expected), object: field)
        let stopResult = await XCTWaiter.fulfillment(of: [completed], timeout: 10)
        XCTAssertEqual(stopResult, .completed)
        guard stopResult == .completed else { return }
        XCTAssertTrue(dictate.waitForExistence(timeout: 5))
        XCTAssertFalse(recording.exists)
        field.tap()
        field.typeText(" edited")
        XCTAssertEqual(field.value as? String, expected + " edited")
        // Cancel a second recording and reject its late final text entirely.
        dictate.tap()
        XCTAssertTrue(recording.waitForExistence(timeout: 5))
        element("composer-dictation-cancel").tap()
        XCTAssertTrue(dictate.waitForExistence(timeout: 5))
        XCTAssertEqual(field.value as? String, expected + " edited")
        XCTAssertTrue(element("composer-send").isEnabled)
        XCTAssertFalse(speech.exists)
        field.typeText(" The complete draft stays visible while I pause and think. The controls stay below the text instead of squeezing it between the buttons.")
        XCTAssertGreaterThan(field.frame.height, 60, "A long draft must expand into multiple readable lines.")
        XCTAssertGreaterThan(field.frame.width, app.frame.width * 0.85, "An expanded draft must use the composer width, not the gap between buttons.")
        screenshot("dictation-multiline-draft")
        let after: [Message] = try await request("/channels/\(fixture.generalChannelId)/messages")
        XCTAssertEqual(Set(after.map(\.id)), Set(before.map(\.id)), "Dictation and stopping must never send a message.")
    }

    private func permissionButton(system: XCUIApplication, title: XCUIElement, labels: [String]) -> XCUIElement {
        // Only containers with this exact visible title may supply the action.
        system.descendants(matching: .any).containing(.staticText, identifier: title.label)
            .buttons.matching(NSPredicate(format: "label IN %@", labels)).firstMatch
    }

    private func consumeDictationPrivacyReset() throws {
        let environment = ProcessInfo.processInfo.environment
        let simulator = try XCTUnwrap(environment["SIMULATOR_UDID"].flatMap(UUID.init(uuidString:)),
                                      "The first-run regression requires an identified simulator, never a physical microphone.")
        let credentials = URL(fileURLWithPath: fixturePath).resolvingSymlinksInPath()
        let receiptURL = URL(fileURLWithPath: credentials.path + ".dictation-reset.json")
        guard FileManager.default.fileExists(atPath: receiptURL.path) else {
            XCTFail("First-run dictation requires a fresh privacy reset. Use apps/ios/scripts/test-first-run-dictation.py or ci_scripts/reset-dictation-privacy.py before this test invocation.")
            throw FixtureError.invalidLocalFixture
        }
        let attributes = try receiptURL.resourceValues(forKeys: [.isRegularFileKey, .isSymbolicLinkKey])
        guard attributes.isRegularFile == true, attributes.isSymbolicLink == false else {
            XCTFail("The privacy reset receipt must be a regular file, not a symlink.")
            throw FixtureError.invalidLocalFixture
        }
        let data = try Data(contentsOf: receiptURL)
        let receipt = try JSONDecoder().decode(DictationPrivacyReset.self, from: data)
        let age = Date().timeIntervalSince1970 - receipt.resetAt
        // Python resolves /tmp to /private/tmp. Normalize both paths with the same
        // Foundation API instead of comparing a host spelling with a simulator spelling.
        let receiptCredentials = URL(fileURLWithPath: receipt.fixturePath).resolvingSymlinksInPath()
        let expectedCommand = ["/usr/bin/xcrun", "simctl", "privacy", receipt.simulatorID, "reset", "all", "app.denchat.ios"]
        var mismatches: [String] = []
        if receipt.bundleID != "app.denchat.ios" {
            mismatches.append("bundle expected app.denchat.ios, receipt \(receipt.bundleID)")
        }
        if UUID(uuidString: receipt.simulatorID) != simulator {
            mismatches.append("simulator expected \(simulator.uuidString), receipt \(receipt.simulatorID)")
        }
        if receiptCredentials.path != credentials.path {
            mismatches.append("fixture expected \(credentials.path), receipt canonical \(receiptCredentials.path), receipt original \(receipt.fixturePath)")
        }
        if receipt.resetExitCode != 0 {
            mismatches.append("reset exit expected 0, receipt \(receipt.resetExitCode)")
        }
        if receipt.command != expectedCommand {
            mismatches.append("command expected \(expectedCommand), receipt \(receipt.command)")
        }
        if !(-5...1800).contains(age) {
            mismatches.append("reset age expected -5...1800 seconds, actual \(age)")
        }
        guard mismatches.isEmpty else {
            XCTFail("Privacy reset receipt mismatch: " + mismatches.joined(separator: "; "))
            throw FixtureError.invalidLocalFixture
        }
        let nonce = try XCTUnwrap(UUID(uuidString: receipt.nonce))
        // Atomic single-use consumption also rejects a rerun that forgot to reset privacy.
        let consumed = URL(fileURLWithPath: credentials.path + ".dictation-reset-consumed-" + nonce.uuidString + ".json")
        try FileManager.default.moveItem(at: receiptURL, to: consumed)
        let attachment = XCTAttachment(data: data, uniformTypeIdentifier: "public.json")
        attachment.name = "dictation-privacy-reset-receipt"
        attachment.lifetime = .keepAlways
        add(attachment)
    }

    private struct DictationPrivacyReset: Decodable {
        let nonce: String
        let bundleID: String
        let simulatorID: String
        let fixturePath: String
        let resetAt: Double
        let resetExitCode: Int
        let command: [String]
    }

    func testNativeTextFlowAndSessionRestoration() async throws {
        try configureFixture()
        // Each run must actually change appearance, even when a prior interrupted
        // test left this disposable account on the target theme.
        let _: Appearance = try await request("/users/me/appearance", method: "PUT",
                                             body: ["light_theme": "den", "dark_theme": "den", "mode": "light", "custom_themes": [],
                                                    "background": appearanceBackground, "contrast": 110])
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
        if !editor.waitForExistence(timeout: 3) { contextAction("Edit", messageId: first.id) }
        XCTAssertTrue(editor.waitForExistence(timeout: 5))
        editor.tap()
        if !app.keyboards.firstMatch.waitForExistence(timeout: 2) { editor.tap() }
        XCTAssertTrue(app.keyboards.firstMatch.waitForExistence(timeout: 5))
        editor.typeText(" revised")
        app.buttons["Save"].tap()
        let edited = try await waitForMessage(id: first.id) { $0.content != original }
        XCTAssertEqual(edited.content.replacingOccurrences(of: " revised", with: ""), original)
        XCTAssertTrue(edited.content.contains(" revised"))

        contextAction("Reply", messageId: first.id)
        XCTAssertTrue(element("thread-timeline").waitForExistence(timeout: 5))
        XCTAssertTrue(element("message-" + first.id).waitForExistence(timeout: 5))
        let replyText = marker + " reply"
        send(replyText)
        let reply = try await waitForMessage(channel: fixture.generalChannelId, content: replyText)
        XCTAssertEqual(reply.replyTo, first.id)
        XCTAssertNotNil(reply.threadId, "Replying to a room root must create its conversation.")
        XCTAssertTrue(element("message-" + reply.id).waitForExistence(timeout: 10))
        leaveThread()

        contextAction("React", messageId: first.id)
        XCTAssertTrue(app.buttons["👍"].waitForExistence(timeout: 5))
        app.buttons["👍"].tap()
        let reacted = try await waitForMessage(id: first.id) { message in
            message.reactions.contains { $0.emoji == "👍" && $0.userIds.contains(self.fixture.users[0].session.user.id) }
        }
        XCTAssertEqual(reacted.reactions.first { $0.emoji == "👍" }?.userIds.filter { $0 == fixture.users[0].session.user.id }.count, 1)
        let reactionID = "reaction-\(first.id)-👍"
        let reactionMatches = element("conversation-timeline").descendants(matching: .button)
            .matching(identifier: reactionID)
        XCTAssertTrue(reactionMatches.firstMatch.waitForExistence(timeout: 5))
        let timeline = element("conversation-timeline")
        dismissKeyboard(in: timeline)
        var reaction = reactionMatches.allElementsBoundByIndex.first(where: \.isHittable)
        for _ in 0..<3 where reaction == nil {
            timeline.swipeUp()
            reaction = reactionMatches.allElementsBoundByIndex.first(where: \.isHittable)
        }
        let tappableReaction = try XCTUnwrap(reaction, "The room's reaction control must be tappable.")
        tappableReaction.tap()
        _ = try await waitForMessage(id: first.id) { message in
            !message.reactions.contains { $0.emoji == "👍" && $0.userIds.contains(self.fixture.users[0].session.user.id) }
        }
        screenshot("m5a-conversation-reply")

        let disposableText = marker + " delete me"
        send(disposableText)
        let disposable = try await waitForMessage(channel: fixture.generalChannelId, content: disposableText)
        contextAction("Delete", messageId: disposable.id)
        XCTAssertTrue(app.buttons["Delete message"].waitForExistence(timeout: 5))
        app.buttons["Delete message"].tap()
        try await waitForDeletion(disposable.id)
        let deletedRow = XCTNSPredicateExpectation(predicate: NSPredicate(format: "exists == false"), object: element("message-" + disposable.id))
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
        XCTAssertTrue(tabButton("Rooms").isSelected)

        searchFor(marker)
        let result = element("search-result-" + reply.id)
        XCTAssertTrue(result.waitForExistence(timeout: 10))
        result.tap()
        XCTAssertTrue(element("message-" + reply.id).waitForExistence(timeout: 10))
        XCTAssertTrue(tabButton("Rooms").isSelected)

        selectTab("Settings")
        app.buttons["Appearance"].tap()
        let tide = element("theme-tide")
        XCTAssertTrue(tide.waitForExistence(timeout: 5))
        tide.tap()
        try await waitForAppearance(light: "tide", dark: "den", mode: "light")
        app.buttons["Dark"].tap()
        try await waitForAppearance(light: "tide", dark: "den", mode: "dark")
        tide.tap()
        try await waitForAppearance(light: "tide", dark: "tide", mode: "dark")
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
        XCTAssertFalse(tabButton("Rooms").exists)
    }

    func testSearchResultRestoresUsableTabNavigation() async throws {
        try configureFixture()
        let marker = "iossearch" + UUID().uuidString.replacingOccurrences(of: "-", with: "").lowercased()
        let message = try await postMessage(channel: fixture.generalChannelId, content: marker, user: 0)
        defer { app.terminate() }
        do {
            app.launch()
            XCTAssertTrue(element("login-submit").waitForExistence(timeout: 10))
            element("login-submit").tap()
            try await waitForRooms()
            searchFor(marker)
            let result = element("search-result-" + message.id)
            XCTAssertTrue(result.waitForExistence(timeout: 10))
            screenshot("m5b-search-results")
            result.tap()
            XCTAssertTrue(element("message-" + message.id).waitForExistence(timeout: 10))
            let rooms = tabButton("Rooms")
            guard rooms.waitForExistence(timeout: 5) else {
                XCTFail("Opening a search result must restore the app's tab controls.")
                throw FixtureError.timedOut
            }
            XCTAssertTrue(rooms.isSelected)
            XCTAssertTrue(rooms.isHittable)
            screenshot("m5b-search-navigation")
            selectTab("Settings")
            XCTAssertTrue(element("account-logout").waitForExistence(timeout: 5))
        } catch {
            _ = try? await rawRequest("/messages/" + message.id, method: "DELETE")
            throw error
        }
        let (_, deleted) = try await rawRequest("/messages/" + message.id, method: "DELETE")
        XCTAssertEqual(deleted, 204)
    }

    func testNativeThreadLifecycleUnreadAndDeepLinks() async throws {
        try configureFixture()
        let marker = "iosthread" + UUID().uuidString.replacingOccurrences(of: "-", with: "").lowercased()
        let root = try await postMessage(channel: fixture.generalChannelId,
                                         content: marker + " root topic", user: 0)
        let replyText = marker + " first reply"
        let preseededReply: Message?
        if ProcessInfo.processInfo.environment["DEN_UI_PRESEED_THREAD"] == "1" {
            preseededReply = try await request("/channels/\(fixture.generalChannelId)/messages", method: "POST",
                                                body: ["content": replyText, "reply_to": root.id], user: 0)
        } else {
            preseededReply = nil
        }
        if ProcessInfo.processInfo.environment["DEN_UI_LANDSCAPE"] == "1" {
            XCUIDevice.shared.orientation = .landscapeLeft
        }
        defer {
            app.terminate()
            XCUIDevice.shared.orientation = .portrait
        }

        app.launch()
        XCTAssertTrue(element("login-submit").waitForExistence(timeout: 10))
        element("login-submit").tap()
        try await waitForRooms()
        openRoom(fixture.generalChannelId)

        let reply: Message
        if let preseededReply {
            let threadId = try XCTUnwrap(preseededReply.threadId)
            let seededChip = element("thread-chip-" + threadId)
            XCTAssertTrue(seededChip.waitForExistence(timeout: 10))
            seededChip.tap()
            XCTAssertTrue(element("thread-timeline").waitForExistence(timeout: 10))
            reply = preseededReply
        } else {
            XCTAssertTrue(element("message-" + root.id).waitForExistence(timeout: 10))
            contextAction("Reply", messageId: root.id)
            XCTAssertTrue(element("thread-timeline").waitForExistence(timeout: 10))
            send(replyText)
            reply = try await waitForMessage(channel: fixture.generalChannelId, content: replyText)
        }
        let threadId = try XCTUnwrap(reply.threadId)
        XCTAssertTrue(element("message-" + reply.id).waitForExistence(timeout: 10))
        screenshot("native-thread-conversation")

        leaveThread()
        let chip = element("thread-chip-" + threadId)
        XCTAssertTrue(chip.waitForExistence(timeout: 10), "The room must expose a newly created conversation without a reload.")
        chip.tap()
        XCTAssertTrue(element("thread-timeline").waitForExistence(timeout: 10))

        element("thread-actions").tap()
        XCTAssertTrue(element("thread-action-rename").waitForExistence(timeout: 5))
        element("thread-action-rename").tap()
        let rename = app.alerts["Rename conversation"].textFields["Conversation title"]
        XCTAssertTrue(rename.waitForExistence(timeout: 5))
        rename.tap()
        rename.typeText(" · renamed")
        app.buttons["Save"].tap()
        let renamed = try await waitForThread(id: threadId) { $0.thread.title.contains("renamed") }
        XCTAssertTrue(renamed.thread.title.contains("renamed"))

        element("thread-actions").tap()
        XCTAssertTrue(element("thread-action-follow").waitForExistence(timeout: 5))
        element("thread-action-follow").tap()
        _ = try await waitForThread(id: threadId) { !$0.readState.following }
        element("thread-actions").tap()
        XCTAssertTrue(element("thread-action-follow").waitForExistence(timeout: 5))
        element("thread-action-follow").tap()
        _ = try await waitForThread(id: threadId) { $0.readState.following }

        element("thread-actions").tap()
        XCTAssertTrue(element("thread-action-resolve").waitForExistence(timeout: 5))
        element("thread-action-resolve").tap()
        _ = try await waitForThread(id: threadId) { $0.thread.resolvedAt != nil }
        XCTAssertTrue(element("thread-reopen").waitForExistence(timeout: 10))
        XCTAssertFalse(element("composer-field").exists)
        XCUIDevice.shared.press(.home)
        app.activate()
        XCTAssertTrue(element("thread-reopen").waitForExistence(timeout: 10),
                      "Resolved lifecycle state must survive foreground restoration.")
        element("thread-reopen").tap()
        _ = try await waitForThread(id: threadId) { $0.thread.resolvedAt == nil }
        XCTAssertTrue(element("composer-field").waitForExistence(timeout: 10))

        leaveThread()
        if case nil = preseededReply {
            XCTAssertTrue(element("message-" + root.id).waitForExistence(timeout: 10))
        }
        let remoteText = marker + " resolved unread reply"
        let remote: Message = try await request("/channels/\(fixture.generalChannelId)/messages", method: "POST",
                                                body: ["content": remoteText, "thread_id": threadId], user: 1)
        let _: ThreadSummaryProjection = try await request("/threads/\(threadId)", method: "PATCH",
                                                           body: ["resolved": true], user: 1)
        selectTab("Inbox")
        let unreadThread = element("inbox-thread-" + threadId)
        XCTAssertTrue(unreadThread.waitForExistence(timeout: 15),
                      "Unread resolved conversations must stay reachable from Inbox.")
        screenshot("native-thread-unread-resolved-inbox")
        unreadThread.tap()
        XCTAssertTrue(element("message-" + remote.id).waitForExistence(timeout: 10))
        XCTAssertTrue(element("thread-reopen").waitForExistence(timeout: 10))
        _ = try await waitForThread(id: threadId) { $0.readState.unreadCount == 0 }

        searchFor(replyText)
        let result = element("search-result-" + reply.id)
        if ProcessInfo.processInfo.environment["DEN_UI_ACCESSIBILITY_TEXT"] == "1" {
            let list = element("search-list")
            XCTAssertTrue(list.waitForExistence(timeout: 5))
            for _ in 0..<5 where !result.exists { list.swipeUp() }
        }
        XCTAssertTrue(result.waitForExistence(timeout: 10))
        result.tap()
        XCTAssertTrue(element("thread-timeline").waitForExistence(timeout: 10))
        XCTAssertTrue(element("message-" + reply.id).waitForExistence(timeout: 10))
        XCTAssertTrue(tabButton("Rooms").isSelected)
    }

    private func searchFor(_ text: String) {
        selectTab("Search")
        let search = app.textFields["search-query"]
        XCTAssertTrue(search.waitForExistence(timeout: 5))
        XCTAssertTrue(search.isHittable)
        search.tap()
        search.typeText(text)
        let submit = element("search-submit")
        XCTAssertTrue(submit.waitForExistence(timeout: 5))
        XCTAssertTrue(submit.isEnabled)
        submit.tap()
        if ProcessInfo.processInfo.environment["DEN_UI_ACCESSIBILITY_TEXT"] == "1" {
            element("search-list").swipeUp()
        }
    }

    private func waitForRooms() async throws {
        let rooms = tabButton("Rooms")
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

    private func leaveThread() {
        let back = element("thread-back")
        XCTAssertTrue(back.waitForExistence(timeout: 5))
        back.tap()
        XCTAssertTrue(element("conversation-timeline").waitForExistence(timeout: 10))
    }

    private func showRoomsList() {
        selectTab("Rooms")
        if !element("new-dm").exists { app.navigationBars.buttons.element(boundBy: 0).tap() }
        XCTAssertTrue(element("new-dm").waitForExistence(timeout: 5))
    }

    // iPad's floating tab strip exposes buttons without a TabBar ancestor.
    // Keep exact labels and selected/hittable assertions on both device layouts.
    private func tabButton(_ name: String) -> XCUIElement {
        app.buttons.matching(NSPredicate(format: "label == %@", name)).firstMatch
    }

    private func selectTab(_ name: String) {
        if app.keyboards.firstMatch.exists {
            let timeline = element("conversation-timeline")
            XCTAssertTrue(timeline.waitForExistence(timeout: 5))
            dismissKeyboard(in: timeline)
        }
        let tab = tabButton(name)
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
        let timeline = element("conversation-timeline")
        dismissKeyboard(in: timeline)
        let message = element("message-" + messageId)
        for _ in 0..<3 where !message.exists { timeline.swipeUp() }
        XCTAssertTrue(message.waitForExistence(timeout: 10))
        if !message.isHittable { timeline.swipeUp() }
        message.press(forDuration: 1.1)
        let button = app.buttons[action]
        if !button.waitForExistence(timeout: 3) {
            message.tap()
            message.press(forDuration: 1.1)
        }
        XCTAssertTrue(button.waitForExistence(timeout: 5))
        button.tap()
    }

    private func dismissKeyboard(in timeline: XCUIElement) {
        guard app.keyboards.firstMatch.exists else { return }
        let visibleMessage = app.descendants(matching: .any)
            .matching(NSPredicate(format: "identifier BEGINSWITH %@", "message-"))
            .allElementsBoundByIndex.first(where: \.isHittable)
        if let visibleMessage { visibleMessage.tap() }
        if app.keyboards.firstMatch.exists {
            timeline.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.2)).tap()
        }
        let hidden = XCTNSPredicateExpectation(predicate: NSPredicate(format: "exists == false"), object: app.keyboards.firstMatch)
        XCTAssertEqual(XCTWaiter.wait(for: [hidden], timeout: 5), .completed)
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

    private func waitForAppearance(light: String, dark: String, mode: String) async throws {
        for _ in 0..<50 {
            let (data, status) = try await rawRequest("/users/me/appearance")
            XCTAssertEqual(status, 200)
            let appearance = try JSONDecoder().decode(Appearance.self, from: data)
            let value = try XCTUnwrap(JSONSerialization.jsonObject(with: data) as? [String: Any])
            if appearance.lightTheme == light && appearance.darkTheme == dark && appearance.mode == mode {
                XCTAssertEqual(value["contrast"] as? Int, 110, "Changing theme or mode must preserve the account's contrast setting.")
                XCTAssertEqual(value["background"] as? NSDictionary, appearanceBackground as NSDictionary,
                               "Changing theme or mode must not erase a background selected in another client.")
                return
            }
            try await Task.sleep(for: .milliseconds(200))
        }
        XCTFail("The server did not save the selected appearance.")
        throw FixtureError.timedOut
    }

    private func waitForThread(id: String, matching predicate: (ThreadViewProjection) -> Bool) async throws -> ThreadViewProjection {
        for _ in 0..<50 {
            let value: ThreadViewProjection = try await request("/threads/\(id)")
            if predicate(value) { return value }
            try await Task.sleep(for: .milliseconds(200))
        }
        XCTFail("The server did not persist the expected conversation state.")
        throw FixtureError.timedOut
    }

    private var appearanceBackground: [String: Any] {
        ["source": ["type": "builtin", "name": "aurora"], "blur": 8, "dim": 20,
         "saturate": 100, "scope": "chat", "fit": "cover"]
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
        let threadId: String?
        let reactions: [Reaction]
        enum CodingKeys: String, CodingKey {
            case id, content, reactions
            case channelId = "channel_id", replyTo = "reply_to", threadId = "thread_id"
        }
        struct Reaction: Decodable {
            let emoji: String
            let userIds: [String]
            enum CodingKeys: String, CodingKey { case emoji; case userIds = "user_ids" }
        }
    }
    private struct ThreadViewProjection: Decodable {
        let thread: ThreadSummaryProjection
        let readState: ThreadReadProjection
        enum CodingKeys: String, CodingKey { case thread; case readState = "read_state" }
    }
    private struct ThreadSummaryProjection: Decodable {
        let id: String
        let title: String
        let resolvedAt: String?
        enum CodingKeys: String, CodingKey { case id, title; case resolvedAt = "resolved_at" }
    }
    private struct ThreadReadProjection: Decodable {
        let following: Bool
        let unreadCount: Int
        enum CodingKeys: String, CodingKey { case following; case unreadCount = "unread_count" }
    }
    private struct Appearance: Decodable {
        let lightTheme: String; let darkTheme: String; let mode: String
        enum CodingKeys: String, CodingKey { case lightTheme = "light_theme", darkTheme = "dark_theme", mode }
    }
}
