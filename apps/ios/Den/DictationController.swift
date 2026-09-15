import Foundation
import Observation
import UIKit

@MainActor protocol DictationSessionDriver: AnyObject {
    var audioLevel: Double { get }
    var recordingDuration: TimeInterval { get }
    func prepare() async throws
    func startCapture() throws
    /// Stops all hardware synchronously. Async completion must never touch AVAudioSession.
    func stopCapture()
    func finish() async
    func cancel() -> Task<Void, Never>
}

extension DictationSessionDriver {
    var audioLevel: Double { 0 }
    var recordingDuration: TimeInterval { 0 }
}

@MainActor @Observable final class DictationController {
    struct Language: Identifiable, Equatable, Sendable {
        let id: String
        let name: String
        let analyzer: Bool
        let legacy: Bool
    }
    enum State { case idle, preparing, listening, finishing }
    enum Authorization { case allowed, microphoneDenied, cancelled }
    struct Dependencies {
        var discover: @MainActor () async -> [Language]
        var authorize: @MainActor (@escaping @MainActor () -> Bool) async -> Authorization
        var makeDriver: @MainActor (Language, Bool, @escaping @MainActor (String) -> Void,
                                   @escaping @MainActor (Bool) -> Void) -> any DictationSessionDriver
        var finishTimeout: Duration = .seconds(60)
    }

    private(set) var languages: [Language] = []
    private(set) var state = State.idle
    private(set) var error: String?
    private(set) var audioLevel: Double = 0
    private(set) var recordingDuration: TimeInterval = 0
    var selectedLanguageID: String {
        didSet {
            guard selectedLanguageID != oldValue else { return }
            cancel(); defaults.set(selectedLanguageID, forKey: "dictation.language")
        }
    }
    var punctuationEnabled: Bool {
        didSet {
            guard punctuationEnabled != oldValue else { return }
            cancel(); defaults.set(punctuationEnabled, forKey: "dictation.punctuation")
        }
    }
    var isActive: Bool { state != .idle }
    /// Snapshot when queuing composer work; invalidation also cancels work not started yet.
    var contextRevision: Int { generation }
    var isAvailable: Bool { selectedLanguage != nil && !isCallActive() }
    var supportsPunctuation: Bool { selectedLanguage.map { !$0.analyzer && $0.legacy } ?? false }

    @ObservationIgnored private let defaults: UserDefaults
    @ObservationIgnored private let isCallActive: @MainActor () -> Bool
    @ObservationIgnored private let dependencies: Dependencies
    @ObservationIgnored private var generation = 0
    @ObservationIgnored private var hasDiscoveredLanguages = false
    @ObservationIgnored private var discoveryTask: Task<Void, Never>?
    @ObservationIgnored private var driver: (any DictationSessionDriver)?
    @ObservationIgnored private var cleanup: Task<Void, Never>?
    @ObservationIgnored private var finishTask: Task<Void, Never>?
    @ObservationIgnored private var finishDeadline: Task<Void, Never>?
    @ObservationIgnored private var meterTask: Task<Void, Never>?
    @ObservationIgnored private var transcriptHandler: (@MainActor (String) -> Void)?
    @ObservationIgnored private var lastTranscript = ""
    private var selectedLanguage: Language? { languages.first { $0.id == selectedLanguageID } }

    convenience init(isCallActive: @escaping @MainActor () -> Bool) {
        self.init(defaults: .standard, isCallActive: isCallActive, dependencies: .live)
    }
    init(defaults: UserDefaults, isCallActive: @escaping @MainActor () -> Bool, dependencies: Dependencies) {
        self.defaults = defaults; self.isCallActive = isCallActive; self.dependencies = dependencies
        selectedLanguageID = defaults.string(forKey: "dictation.language")
            ?? Locale(identifier: Locale.preferredLanguages.first ?? Locale.current.identifier).identifier(.bcp47)
        punctuationEnabled = defaults.object(forKey: "dictation.punctuation") as? Bool ?? true
    }

    /// Room recreation reuses even an empty result. Settings can explicitly check again.
    func refresh(force: Bool = false) async {
        if let discoveryTask { await discoveryTask.value; return }
        guard force || !hasDiscoveredLanguages else { return }
        // Shared work outlives a canceled view task; no recognizer or transcript is retained.
        let task = Task { [weak self, discover = dependencies.discover] in
            let available = await discover()
            guard let self else { return }
            self.languages = available
            self.hasDiscoveredLanguages = true
            self.discoveryTask = nil
            // Match a regional spelling, never silently choose an unrelated language.
            if !self.isActive, self.selectedLanguage == nil,
               let match = available.first(where: {
                   Locale(identifier: $0.id).language.languageCode == Locale(identifier: self.selectedLanguageID).language.languageCode
               }) { self.selectedLanguageID = match.id }
        }
        discoveryTask = task
        await task.value
    }

    /// Recording never changes the draft. Deliver the complete text once after explicit stop.
    func start(onTranscript: @escaping @MainActor (String) -> Void) async {
        guard !isActive, !isCallActive() else { return }
        generation += 1
        let expected = generation
        state = .preparing; error = nil; lastTranscript = ""; transcriptHandler = onTranscript
        audioLevel = 0; recordingDuration = 0
        await cleanup?.value
        guard owns(expected) else { return }
        let authorization = await dependencies.authorize { [weak self] in self?.owns(expected) ?? false }
        guard owns(expected) else { return }
        switch authorization {
        case .microphoneDenied: fail("Den needs the microphone for dictation. Allow it in Settings.", expected: expected); return
        case .cancelled: cancel(); return
        case .allowed: break
        }
        await refresh()
        guard owns(expected) else { return }
        guard let language = selectedLanguage else { fail("On-device dictation isn't available for this language.", expected: expected); return }
        let session = dependencies.makeDriver(language, punctuationEnabled, { [weak self] text in
            self?.receive(text, expected: expected)
        }, { [weak self] failed in
            guard let self, self.owns(expected) else { return }
            if failed { self.fail("Dictation stopped. Your text is still here. Try again.", expected: expected) }
            else { self.complete(expected) }
        })
        driver = session
        do {
            try await session.prepare()
            guard owns(expected), state == .preparing else { return }
            try session.startCapture()
            state = .listening
            meterTask = Task { [weak self] in
                while !Task.isCancelled {
                    guard let self, self.owns(expected), self.state == .listening else { return }
                    self.audioLevel = session.audioLevel
                    self.recordingDuration = session.recordingDuration
                    if self.recordingDuration >= 300 { self.stop(); return }
                    try? await Task.sleep(for: .milliseconds(80))
                }
            }
            UIImpactFeedbackGenerator(style: .light).impactOccurred()
        } catch {
            guard owns(expected) else { return }
            fail("Couldn't start on-device dictation. Try again.", expected: expected)
        }
    }

    func stop() {
        guard state != .idle else { cancel(); return }
        guard state != .finishing else { return }
        guard state == .listening, let driver else { cancel(); return }
        driver.stopCapture() // Before returning, including before any future CallKit preparation.
        meterTask?.cancel(); meterTask = nil
        audioLevel = 0; recordingDuration = driver.recordingDuration
        state = .finishing
        UIImpactFeedbackGenerator(style: .light).impactOccurred()
        let expected = generation
        finishTask = Task { [weak self] in
            await driver.finish()
            guard let self, self.owns(expected) else { return }
            self.complete(expected)
        }
        finishDeadline = Task { [weak self] in
            try? await Task.sleep(for: self?.dependencies.finishTimeout ?? .seconds(60))
            guard !Task.isCancelled, let self, self.owns(expected) else { return }
            self.fail("Transcription took too long. Your draft is still here. Try a shorter recording.", expected: expected)
        }
    }

    /// Call before manual text/selection changes, send, navigation, logout or call preparation.
    func invalidateContext() { cancel() }
    func cancel() {
        let wasListening = state == .listening
        generation += 1 // Drop late permission, framework and finalization callbacks first.
        transcriptHandler = nil
        lastTranscript = ""
        meterTask?.cancel(); meterTask = nil
        finishTask?.cancel(); finishTask = nil
        finishDeadline?.cancel(); finishDeadline = nil
        if let driver {
            driver.stopCapture()
            let priorCleanup = cleanup
            let currentCleanup = driver.cancel()
            cleanup = Task { await priorCleanup?.value; await currentCleanup.value }
        }
        driver = nil; state = .idle; audioLevel = 0; recordingDuration = 0
        if wasListening { UIImpactFeedbackGenerator(style: .light).impactOccurred() }
    }
    func clearError() { error = nil }

    private func owns(_ expected: Int) -> Bool {
        guard generation == expected, state != .idle else { return false }
        if isCallActive() || Task.isCancelled { cancel(); return false }
        return true
    }
    private func receive(_ text: String, expected: Int) {
        guard owns(expected), text != lastTranscript else { return }
        lastTranscript = text
    }
    private func fail(_ message: String, expected: Int) {
        guard owns(expected) else { return }
        cancel(); error = message
    }
    private func complete(_ expected: Int) {
        guard owns(expected) else { return }
        guard state == .finishing else { return }
        let text = lastTranscript
        let handler = transcriptHandler
        cancel()
        if !text.isEmpty { handler?(text) }
    }
}
