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
        // Never introduce a second prompt for local dictation. The legacy fallback
        // is available only where the user has already authorized that API.
        if SFSpeechRecognizer.authorizationStatus() == .authorized {
            for locale in SFSpeechRecognizer.supportedLocales() {
                if SFSpeechRecognizer(locale: locale)?.supportsOnDeviceRecognition == true {
                    legacyIDs.insert(locale.identifier(.bcp47))
                }
            }
        }
        return analyzerIDs.union(legacyIDs).map { id in
            DictationController.Language(id: id, name: Locale.current.localizedString(forIdentifier: id) ?? id,
                                         analyzer: analyzerIDs.contains(id), legacy: legacyIDs.contains(id))
        }.sorted { $0.name.localizedStandardCompare($1.name) == .orderedAscending }
    }

    static func authorize(isCurrent: @escaping @MainActor () -> Bool) async -> DictationController.Authorization {
        let microphone = await withCheckedContinuation { continuation in
            AVAudioApplication.requestRecordPermission { @Sendable in continuation.resume(returning: $0) }
        }
        guard microphone else { return .microphoneDenied }
        guard isCurrent() else { return .cancelled }
        do { try await DictationAudioCapture.waitForForeground() }
        catch { return .cancelled }
        // SpeechAnalyzer transcriber modules need microphone permission, not
        // SFSpeechRecognizer's legacy authorization to use Apple's speech service.
        return .allowed
    }

    /// Both guards are mandatory: the request flag alone is not honored on unsupported devices.
    static func legacyRequest(url: URL, supportsOnDeviceRecognition: Bool, punctuation: Bool) throws -> SFSpeechURLRecognitionRequest {
        guard supportsOnDeviceRecognition else { throw DictationAudioError.unavailable }
        let request = SFSpeechURLRecognitionRequest(url: url)
        request.requiresOnDeviceRecognition = true
        request.shouldReportPartialResults = false
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
    private var fileTranscription: DictationFileTranscription?
    private var transcriber: SpeechTranscriber?
    private var recording: DictationRecording?
    private var audioTask: Task<Void, Never>?
    private var recognizer: SFSpeechRecognizer?
    private var legacyTask: SFSpeechRecognitionTask?
    private var legacyResult: CheckedContinuation<String, Error>?
    var audioLevel: Double { recording?.audioLevel ?? 0 }
    var recordingDuration: TimeInterval { recording?.duration ?? 0 }

    init(language: DictationController.Language, punctuation: Bool,
         onTranscript: @escaping @MainActor (String) -> Void, onEnd: @escaping @MainActor (Bool) -> Void) {
        self.language = language; self.punctuation = punctuation
        self.onTranscript = onTranscript; self.onEnd = onEnd
    }

    func prepare() async throws {
        if language.analyzer {
            do {
                let module = try await Self.installedTranscriber(locale: Locale(identifier: language.id))
                try check()
                let preferred = await SpeechAnalyzer.bestAvailableAudioFormat(compatibleWith: [module])
                let compatible = await module.availableCompatibleAudioFormats
                try check()
                let format = try DictationAudioConverter.analyzerFormat(preferred: preferred, compatibleFormats: compatible)
                recording = try DictationRecording(format: format)
                transcriber = module
                return
            } catch {
                try check()
                guard language.legacy else { throw error }
            }
        }
        // The fallback also records first and only reads the completed file. It never
        // requests legacy permission or starts recognition while the user is speaking.
        guard language.legacy, SFSpeechRecognizer.authorizationStatus() == .authorized,
              let recognizer = SFSpeechRecognizer(locale: Locale(identifier: language.id)),
              recognizer.supportsOnDeviceRecognition, recognizer.isAvailable,
              let format = AVAudioFormat(commonFormat: .pcmFormatInt16, sampleRate: 16_000,
                                         channels: 1, interleaved: false) else { throw DictationAudioError.unavailable }
        self.recognizer = recognizer
        recording = try DictationRecording(format: format)
    }

    static func installedTranscriber(locale: Locale) async throws -> SpeechTranscriber {
        guard SpeechTranscriber.isAvailable else { throw DictationAudioError.unavailable }
        let module = SpeechTranscriber(locale: locale, transcriptionOptions: [], reportingOptions: [], attributeOptions: [])
        // Inspect installed assets only. Never reserve, install or download a model here.
        guard await AssetInventory.status(forModules: [module]) == .installed else { throw DictationAudioError.unavailable }
        return module
    }

    func startCapture() throws {
        try check()
        guard recording != nil else { throw DictationAudioError.unavailable }
        let stream = try capture.start { [weak self] in self?.onEnd(true) }
        audioTask = Task { [weak self] in
            do {
                for try await frame in stream {
                    guard let self else { return }
                    try self.check()
                    try self.recording?.append(frame.buffer)
                }
            } catch { if let self, !self.cancelled, !Task.isCancelled { self.onEnd(true) } }
        }
    }

    func stopCapture() { capture.stop() }

    func finish() async {
        guard !cancelled, !finishing else { return }
        finishing = true
        await audioTask?.value // Drain every captured frame before closing the file.
        do {
            try check()
            guard let recording else { throw DictationAudioError.unavailable }
            defer { recording.remove() }
            let text: String
            if let transcriber {
                let file = try recording.openForReading()
                let transcription = DictationFileTranscription(module: transcriber)
                fileTranscription = transcription
                text = try await transcription.transcribe(file)
            } else {
                recording.close()
                text = try await transcribeLegacy(url: recording.url)
            }
            try check()
            onTranscript(text)
            onEnd(false)
        } catch { if !cancelled, !Task.isCancelled { onEnd(true) } }
    }

    private func transcribeLegacy(url: URL) async throws -> String {
        guard let recognizer else { throw DictationAudioError.unavailable }
        let request = try DictationPlatform.legacyRequest(url: url,
            supportsOnDeviceRecognition: recognizer.supportsOnDeviceRecognition, punctuation: punctuation)
        return try await withCheckedThrowingContinuation { continuation in
            legacyResult = continuation
            legacyTask = recognizer.recognitionTask(with: request) { @Sendable [weak self] result, error in
                let text = result.flatMap { $0.isFinal ? DictationTranscript.legacy($0.bestTranscription.formattedString) : nil }
                let failed = error != nil
                Task { @MainActor [weak self] in
                    guard let self, !self.cancelled, let continuation = self.legacyResult else { return }
                    if let text {
                        self.legacyResult = nil; continuation.resume(returning: text)
                    } else if failed {
                        self.legacyResult = nil; continuation.resume(throwing: DictationAudioError.unavailable)
                    }
                }
            }
        }
    }

    func cancel() -> Task<Void, Never> {
        cancelled = true
        capture.stop() // Synchronous hardware release; async cleanup never deactivates audio.
        audioTask?.cancel()
        legacyTask?.cancel(); legacyTask = nil; recognizer = nil
        legacyResult?.resume(throwing: CancellationError()); legacyResult = nil
        recording?.remove(); recording = nil
        let cleanup = fileTranscription?.cancel() ?? Task {}
        fileTranscription = nil; transcriber = nil
        return cleanup
    }

    private func check() throws {
        guard !cancelled else { throw CancellationError() }
        try Task.checkCancellation()
    }
}

/// Owns one completed file's analyzer and results, including cancellation during inference.
@MainActor final class DictationFileTranscription {
    private let module: SpeechTranscriber
    private let analyzer: SpeechAnalyzer
    private var results: Task<String, Error>?

    init(module: SpeechTranscriber) {
        self.module = module
        analyzer = SpeechAnalyzer(modules: [module], options: .init(priority: .userInitiated, modelRetention: .whileInUse))
    }

    func transcribe(_ file: AVAudioFile) async throws -> String {
        let module = self.module
        let results = Task<String, Error> {
            var transcript = DictationTranscript()
            for try await result in module.results {
                try Task.checkCancellation()
                guard result.isFinal else { continue }
                _ = transcript.analyzer(text: String(result.text.characters), start: result.range.start.seconds,
                                        end: CMTimeRangeGetEnd(result.range).seconds, isFinal: true)
            }
            return transcript.value
        }
        self.results = results
        do {
            let lastSample = try await analyzer.analyzeSequence(from: file)
            try Task.checkCancellation()
            guard let lastSample else {
                results.cancel()
                await analyzer.cancelAndFinishNow()
                return ""
            }
            try await analyzer.finalizeAndFinish(through: lastSample)
            return try await results.value
        } catch {
            results.cancel()
            await analyzer.cancelAndFinishNow()
            throw error
        }
    }

    func cancel() -> Task<Void, Never> {
        results?.cancel()
        return Task { await analyzer.cancelAndFinishNow() }
    }
}
