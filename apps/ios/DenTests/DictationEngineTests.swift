import AVFoundation
import Foundation
import Speech
import Testing
@testable import Den

@Suite(.serialized) struct DictationEngineTests {
    @Test func analyzerRevisionsReplaceVolatileRangesWhileLegacyRemainsCumulative() {
        var transcript = DictationTranscript()
        #expect(transcript.analyzer(text: "Turn left", start: 0, end: 1, isFinal: false) == "Turn left")
        #expect(transcript.analyzer(text: "Turn right.", start: 0, end: 1.2, isFinal: true) == "Turn right.")
        #expect(transcript.analyzer(text: " Then", start: 1.2, end: 2, isFinal: false) == "Turn right. Then")
        #expect(transcript.analyzer(text: " Then stop.", start: 1.2, end: 3, isFinal: true) == "Turn right. Then stop.")
        #expect(transcript.analyzer(text: " Then stop.", start: 1.2, end: 3, isFinal: true) == "Turn right. Then stop.")
        #expect(transcript.analyzer(text: "stale guess", start: 0, end: 1, isFinal: false) == "Turn right. Then stop.")
        #expect(DictationTranscript.legacy("Turn") == "Turn")
        #expect(DictationTranscript.legacy("Turn right.") == "Turn right.", "Legacy partials replace the whole session, never append.")
        var chinese = DictationTranscript()
        #expect(chinese.analyzer(text: "你好", start: 0, end: 1, isFinal: true) == "你好")
        #expect(chinese.analyzer(text: "世界", start: 1, end: 2, isFinal: false) == "你好世界")
    }

    @Test @MainActor func legacyRejectsNetworkOnlyRecognitionAndAlwaysRequiresOnDevice() throws {
        #expect(throws: DictationAudioError.self) {
            try DictationPlatform.legacyRequest(supportsOnDeviceRecognition: false, punctuation: true)
        }
        for punctuation in [false, true] {
            let request = try DictationPlatform.legacyRequest(supportsOnDeviceRecognition: true, punctuation: punctuation)
            #expect(request.requiresOnDeviceRecognition, "A fallback request must never send audio to a service.")
            #expect(request.shouldReportPartialResults)
            #expect(request.addsPunctuation == punctuation)
        }
    }

    @Test(.enabled(if: ProcessInfo.processInfo.environment["DEN_TEST_DICTATION_PERMISSIONS"] == "1",
                   "Opt-in system permission integration: may show microphone and speech prompts; never captures audio."))
    @MainActor func platformPermissionCallbacksResumeOnOwningActor() async {
        var ownershipChecks = 0
        let authorization = await DictationPlatform.authorize(isCurrent: {
            ownershipChecks += 1
            return true
        })
        #expect(AVAudioApplication.shared.recordPermission == .granted)
        #expect(SFSpeechRecognizer.authorizationStatus() == .authorized)
        #expect(authorization == .allowed)
        #expect(ownershipChecks == 1, "Real permission callbacks must resume on the owning context before proceeding.")
    }

    @Test @MainActor func cancellationInvalidatesLatePermissionAndOldSessionCallbacks() async throws {
        let fixture = Fixture()
        defer { fixture.controller.cancel(); fixture.removeDefaults() }
        let english = fixture.availableLanguages
        fixture.availableLanguages = []
        fixture.holdDiscovery = true
        let firstRefresh = Task { await fixture.controller.refresh() }
        await wait { !fixture.discoveryWaiters.isEmpty }
        var secondRefreshStarted = false
        let secondRefresh = Task {
            secondRefreshStarted = true
            await fixture.controller.refresh(force: true)
        }
        await wait { secondRefreshStarted }
        #expect(fixture.discoveryCount == 1, "Concurrent room and Settings refreshes share discovery.")
        firstRefresh.cancel() // Leaving one room must not cancel the shared capability lookup.
        fixture.holdDiscovery = false
        fixture.releaseDiscovery()
        await firstRefresh.value; await secondRefresh.value
        #expect(fixture.controller.languages.isEmpty)
        await fixture.controller.refresh()
        #expect(fixture.discoveryCount == 1, "An unsupported device's empty result is cached too.")
        fixture.availableLanguages = english
        await fixture.controller.refresh(force: true)
        #expect(fixture.discoveryCount == 2, "An explicit Settings refresh checks capabilities again.")
        #expect(fixture.controller.languages == english)

        fixture.holdPermission = true
        var received: [String] = []
        let pending = Task { await fixture.controller.start { received.append($0) } }
        await wait { fixture.permissionWaiter != nil }
        fixture.controller.invalidateContext()
        fixture.holdPermission = false
        fixture.permissionWaiter?.resume(returning: .allowed); fixture.permissionWaiter = nil
        await pending.value
        #expect(fixture.drivers.isEmpty, "A permission response after navigation must not open an audio engine.")

        await fixture.controller.start { received.append($0) }
        let first = try #require(fixture.drivers.last)
        first.emit("first")
        fixture.controller.invalidateContext()
        #expect(!first.capturing, "Context invalidation closes capture synchronously.")
        await fixture.controller.start { received.append($0) }
        let second = try #require(fixture.drivers.last)
        #expect(fixture.discoveryCount == 2, "Starting a new session reuses discovered languages; the driver rechecks readiness.")
        second.emit("second")
        first.emit("late result from first")
        first.end(false)
        #expect(received == ["first", "second"])
        #expect(fixture.controller.state == .listening, "Old completion must not stop the new session.")
        #expect(second.capturing)

        fixture.callActive = true
        second.emit("late result after call became active")
        #expect(!second.capturing)
        #expect(!fixture.controller.isActive)
        #expect(received == ["first", "second"])
        let count = fixture.drivers.count
        await fixture.controller.start { received.append($0) }
        #expect(fixture.drivers.count == count, "Active calls reject dictation before creating capture.")
    }

    @Test @MainActor func stopClosesCaptureBeforeBoundedFinalizationAndEditCancelsPendingFinalText() async throws {
        let fixture = Fixture()
        defer { fixture.controller.cancel(); fixture.drivers.forEach { $0.releaseFinish() }; fixture.removeDefaults() }
        var received: [String] = []
        await fixture.controller.start { received.append($0) }
        let first = try #require(fixture.drivers.last)
        first.emit("keep my last")
        fixture.controller.stop()
        #expect(!first.capturing, "stop() must remove the hardware tap before returning.")
        #expect(fixture.controller.state == .finishing)
        fixture.controller.stop() // An outside tap and focus loss can both report the same stop.
        #expect(fixture.controller.state == .finishing, "A repeated stop must preserve pending final words.")
        await wait { first.finishWaiter != nil }
        first.emit("keep my last word")
        first.releaseFinish()
        await wait { !fixture.controller.isActive }
        #expect(received == ["keep my last", "keep my last word"])

        await fixture.controller.start { received.append($0) }
        let second = try #require(fixture.drivers.last)
        second.emit("editable text")
        fixture.controller.stop()
        await wait { second.finishWaiter != nil }
        fixture.controller.invalidateContext() // A caret move or manual edit owns the text now.
        second.emit("late final must not overwrite the edit")
        second.releaseFinish()
        #expect(received.last == "editable text")
        #expect(!fixture.controller.isActive)

        await fixture.controller.start { received.append($0) }
        let third = try #require(fixture.drivers.last)
        fixture.controller.stop()
        await wait { third.finishWaiter != nil }
        try await Task.sleep(for: .milliseconds(2200))
        #expect(!fixture.controller.isActive, "A recognizer that never finishes must release the session within the deadline.")
        third.emit("too late after finalization deadline")
        #expect(received.last == "editable text")
        third.releaseFinish()
    }

    @MainActor private func wait(_ condition: @MainActor () -> Bool) async {
        for _ in 0..<200 {
            if condition() { return }
            await Task.yield()
        }
        Issue.record("Expected asynchronous dictation transition did not occur.")
    }
}

@MainActor private final class Fixture {
    let name = "DenTests.dictation." + UUID().uuidString
    let defaults: UserDefaults
    var callActive = false
    var holdPermission = false
    var permissionWaiter: CheckedContinuation<DictationController.Authorization, Never>?
    var availableLanguages: [DictationController.Language] = [.init(id: "en-US", name: "English", analyzer: false, legacy: true)]
    var discoveryCount = 0
    var holdDiscovery = false
    var discoveryWaiters: [CheckedContinuation<[DictationController.Language], Never>] = []
    var drivers: [Driver] = []
    lazy var controller = DictationController(defaults: defaults, isCallActive: { [weak self] in self?.callActive ?? true }, dependencies: .init(
        discover: { [weak self] in
            guard let self else { return [] }
            self.discoveryCount += 1
            if self.holdDiscovery { return await withCheckedContinuation { self.discoveryWaiters.append($0) } }
            return self.availableLanguages
        },
        authorize: { [weak self] _ in
            guard let self else { return .microphoneDenied }
            if self.holdPermission { return await withCheckedContinuation { self.permissionWaiter = $0 } }
            return .allowed
        },
        makeDriver: { [weak self] _, _, emit, end in
            let driver = Driver(emit: emit, end: end); self?.drivers.append(driver); return driver
        }))
    init() {
        defaults = UserDefaults(suiteName: name)!
        defaults.set("en-US", forKey: "dictation.language")
    }
    func releaseDiscovery() {
        discoveryWaiters.forEach { $0.resume(returning: availableLanguages) }
        discoveryWaiters.removeAll()
    }
    func removeDefaults() { defaults.removePersistentDomain(forName: name) }
}

@MainActor private final class Driver: DictationSessionDriver {
    let emit: @MainActor (String) -> Void
    let end: @MainActor (Bool) -> Void
    var capturing = false
    var finishWaiter: CheckedContinuation<Void, Never>?
    init(emit: @escaping @MainActor (String) -> Void, end: @escaping @MainActor (Bool) -> Void) {
        self.emit = emit; self.end = end
    }
    func prepare() async throws {}
    func startCapture() throws { capturing = true }
    func stopCapture() { capturing = false }
    func finish() async { await withCheckedContinuation { finishWaiter = $0 } }
    func releaseFinish() { finishWaiter?.resume(); finishWaiter = nil }
    func cancel() -> Task<Void, Never> { stopCapture(); return Task {} }
}
