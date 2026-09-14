import AVFoundation
import CoreMedia
import Speech

extension DictationController.Dependencies {
    @MainActor static var live: Self {
        Self(discover: { await DictationPlatform.languages() }, authorize: { await DictationPlatform.authorize(isCurrent: $0) },
             makeDriver: { NativeDictationSession(language: $0, punctuation: $1, onTranscript: $2, onEnd: $3) })
    }
}

@MainActor enum DictationPlatform {
    static func languages() async -> [DictationController.Language] {
        var analyzerIDs: Set<String> = []
        if SpeechTranscriber.isAvailable {
            for locale in await SpeechTranscriber.installedLocales {
                analyzerIDs.insert(locale.identifier(.bcp47))
            }
        }
        var legacyIDs: Set<String> = []
        for locale in SFSpeechRecognizer.supportedLocales() {
            if SFSpeechRecognizer(locale: locale)?.supportsOnDeviceRecognition == true {
                legacyIDs.insert(locale.identifier(.bcp47))
            }
        }
        return analyzerIDs.union(legacyIDs).map { id in
            DictationController.Language(id: id, name: Locale.current.localizedString(forIdentifier: id) ?? id,
                                         analyzer: analyzerIDs.contains(id), legacy: legacyIDs.contains(id))
        }.sorted { $0.name.localizedStandardCompare($1.name) == .orderedAscending }
    }

    static func authorize(isCurrent: @escaping @MainActor () -> Bool) async -> DictationController.Authorization {
        let microphone = await withCheckedContinuation { continuation in
            AVAudioApplication.requestRecordPermission { continuation.resume(returning: $0) }
        }
        guard microphone else { return .microphoneDenied }
        guard isCurrent() else { return .microphoneDenied }
        let speech: SFSpeechRecognizerAuthorizationStatus = await withCheckedContinuation { continuation in
            SFSpeechRecognizer.requestAuthorization { continuation.resume(returning: $0) }
        }
        return speech == .authorized ? .allowed : .speechDenied
    }

    /// Both guards are mandatory: the request flag alone is not honored on unsupported devices.
    static func legacyRequest(supportsOnDeviceRecognition: Bool, punctuation: Bool) throws -> SFSpeechAudioBufferRecognitionRequest {
        guard supportsOnDeviceRecognition else { throw DictationAudioError.unavailable }
        let request = SFSpeechAudioBufferRecognitionRequest()
        request.requiresOnDeviceRecognition = true
        request.shouldReportPartialResults = true
        request.addsPunctuation = punctuation
        request.taskHint = .dictation
        return request
    }
}

@MainActor final class NativeDictationSession: DictationSessionDriver {
    private let language: DictationController.Language
    private let punctuation: Bool
    private let onTranscript: @MainActor (String) -> Void
    private let onEnd: @MainActor (Bool) -> Void
    private let capture = DictationAudioCapture()
    private var cancelled = false
    private var finishing = false
    private var analyzer: SpeechAnalyzer?
    private var transcriber: SpeechTranscriber?
    private var input: AsyncStream<AnalyzerInput>.Continuation?
    private var converter: DictationAudioConverter?
    private var resultsTask: Task<Void, Never>?
    private var audioTask: Task<Void, Never>?
    private var recognizer: SFSpeechRecognizer?
    private var legacyRequest: SFSpeechAudioBufferRecognitionRequest?
    private var legacyTask: SFSpeechRecognitionTask?
    private var transcript = DictationTranscript()

    init(language: DictationController.Language, punctuation: Bool,
         onTranscript: @escaping @MainActor (String) -> Void, onEnd: @escaping @MainActor (Bool) -> Void) {
        self.language = language; self.punctuation = punctuation
        self.onTranscript = onTranscript; self.onEnd = onEnd
    }

    func prepare() async throws {
        if language.analyzer {
            do { try await prepareAnalyzer(); return }
            catch {
                guard !cancelled, !Task.isCancelled else { throw CancellationError() }
                resultsTask?.cancel()
                await analyzer?.cancelAndFinishNow()
                analyzer = nil; transcriber = nil; input?.finish(); input = nil
                resultsTask?.cancel(); resultsTask = nil
                guard language.legacy else { throw error }
            }
        }
        try prepareLegacy()
    }

    private func prepareAnalyzer() async throws {
        guard SpeechTranscriber.isAvailable else { throw DictationAudioError.unavailable }
        let module = SpeechTranscriber(locale: Locale(identifier: language.id), transcriptionOptions: [],
                                       reportingOptions: [.volatileResults], attributeOptions: [])
        // Inspect installed assets only. Never reserve, install or download a model here.
        guard await AssetInventory.status(forModules: [module]) == .installed else { throw DictationAudioError.unavailable }
        try check()
        guard let format = await SpeechAnalyzer.bestAvailableAudioFormat(compatibleWith: [module]) else { throw DictationAudioError.unavailable }
        try check()
        let analyzer = SpeechAnalyzer(modules: [module], options: .init(priority: .userInitiated, modelRetention: .whileInUse))
        self.analyzer = analyzer; transcriber = module
        converter = DictationAudioConverter(outputFormat: format)
        let pair = AsyncStream<AnalyzerInput>.makeStream(bufferingPolicy: .bufferingOldest(32))
        input = pair.continuation
        try await analyzer.prepareToAnalyze(in: format)
        try check()
        resultsTask = Task { [weak self] in
            do {
                for try await result in module.results {
                    guard let self, !self.cancelled, !Task.isCancelled else { return }
                    let full = self.transcript.analyzer(text: String(result.text.characters), start: result.range.start.seconds,
                                                       end: CMTimeRangeGetEnd(result.range).seconds, isFinal: result.isFinal)
                    self.onTranscript(full)
                }
            } catch { if let self, !self.cancelled, !Task.isCancelled { self.onEnd(true) } }
        }
        try await analyzer.start(inputSequence: pair.stream)
        try check()
    }

    private func prepareLegacy() throws {
        try check()
        guard language.legacy, let recognizer = SFSpeechRecognizer(locale: Locale(identifier: language.id)),
              recognizer.supportsOnDeviceRecognition, recognizer.isAvailable else { throw DictationAudioError.unavailable }
        let request = try DictationPlatform.legacyRequest(supportsOnDeviceRecognition: recognizer.supportsOnDeviceRecognition, punctuation: punctuation)
        self.recognizer = recognizer; legacyRequest = request
        legacyTask = recognizer.recognitionTask(with: request) { @Sendable [weak self] result, error in
            // Extract Sendable values before leaving the framework callback's queue.
            let text = result.map { DictationTranscript.legacy($0.bestTranscription.formattedString) }
            let final = result?.isFinal ?? false
            let failed = error != nil
            Task { @MainActor [weak self] in
                guard let self, !self.cancelled else { return }
                if let text { self.onTranscript(text) }
                if final || failed { self.onEnd(failed && !final) }
            }
        }
    }

    func startCapture() throws {
        try check()
        let stream = try capture.start { [weak self] in self?.onEnd(true) }
        audioTask = Task { [weak self] in
            do {
                for try await frame in stream {
                    guard let self, !self.cancelled, !Task.isCancelled else { return }
                    if let request = self.legacyRequest { request.append(frame.buffer) }
                    else if let converter = self.converter, let buffer = try converter.convert(frame.buffer) {
                        guard let input = self.input else { throw DictationAudioError.unavailable }
                        if case .dropped = input.yield(AnalyzerInput(buffer: buffer)) { throw DictationAudioError.overflow }
                    }
                }
                guard let self else { return }
                self.input?.finish()
                if self.finishing { self.legacyRequest?.endAudio() }
            } catch { if let self, !self.cancelled, !Task.isCancelled { self.onEnd(true) } }
        }
    }

    func stopCapture() { capture.stop() }

    func finish() async {
        guard !cancelled else { return }
        finishing = true
        await audioTask?.value
        input?.finish(); legacyRequest?.endAudio()
        if let analyzer {
            try? await analyzer.finalizeAndFinishThroughEndOfInput()
            await resultsTask?.value
        } else {
            // The controller enforces the two-second deadline. A final callback ends earlier.
            while !cancelled, legacyTask?.state != .completed, !Task.isCancelled {
                try? await Task.sleep(for: .milliseconds(30))
            }
        }
    }

    func cancel() -> Task<Void, Never> {
        cancelled = true
        capture.stop() // Always synchronous; async cleanup below never deactivates audio.
        input?.finish(); input = nil
        audioTask?.cancel(); resultsTask?.cancel()
        legacyRequest?.endAudio(); legacyTask?.cancel()
        legacyTask = nil; legacyRequest = nil; recognizer = nil
        let analyzer = self.analyzer; self.analyzer = nil; transcriber = nil
        return Task { await analyzer?.cancelAndFinishNow() }
    }

    private func check() throws {
        guard !cancelled else { throw CancellationError() }
        try Task.checkCancellation()
    }
}
