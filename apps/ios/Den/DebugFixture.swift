#if DEBUG && targetEnvironment(simulator)
import Foundation
import AVFoundation
import Speech

/// Only the simulator Debug build can prefill a disposable test-server login.
/// The request still goes through the visible sign-in button and real API client.
enum DebugFixture {
    @MainActor static func dictation(isCallActive: @escaping @MainActor () -> Bool) -> DictationController? {
        guard credentials != nil, ProcessInfo.processInfo.arguments.contains("--den-ui-dictation-pcm"),
              let defaults = UserDefaults(suiteName: "DenTests.first-run-dictation") else { return nil }
        defaults.removePersistentDomain(forName: "DenTests.first-run-dictation")
        defaults.set("en-US", forKey: "dictation.language")
        return DictationController(defaults: defaults, isCallActive: isCallActive, dependencies: .init(
            discover: { [.init(id: "en-US", name: "English", analyzer: true, legacy: false)] },
            authorize: { await DictationPlatform.authorize(isCurrent: $0) },
            makeDriver: { _, _, transcript, _ in PCMFrameSession(onTranscript: transcript) }))
    }

    static var credentials: (origin: String, username: String, password: String)? {
        guard ProcessInfo.processInfo.arguments.contains("--den-ui-test"),
              let data = try? Data(contentsOf: URL(fileURLWithPath:
                ProcessInfo.processInfo.environment["DEN_UI_FIXTURE_PATH"] ?? "/tmp/den-ios-qa.json")),
              let fixture = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
              let address = fixture["origin"] as? String,
              let origin = try? ServerOrigin.canonical(address),
              ["127.0.0.1", "localhost", "[::1]"].contains(origin.host() ?? ""),
              let users = fixture["users"] as? [[String: Any]],
              let index = Int(ProcessInfo.processInfo.environment["DEN_UI_USER"] ?? "0"),
              users.indices.contains(index), let username = users[index]["username"] as? String,
              let password = users[index]["password"] as? String else { return nil }
        return (origin.absoluteString, username, password)
    }
}

/// A deterministic source, not a recognizer or permission mock. Never opens hardware.
/// Text is emitted only after the production tap copy and converter accept a PCM frame.
@MainActor private final class PCMFrameSession: DictationSessionDriver {
    private let onTranscript: @MainActor (String) -> Void
    private var delivery: Task<Void, Never>?
    init(onTranscript: @escaping @MainActor (String) -> Void) { self.onTranscript = onTranscript }
    func prepare() async throws {}
    func startCapture() throws {
        try DictationAudioCapture.requireForeground()
        guard let sourceFormat = AVAudioFormat(commonFormat: .pcmFormatFloat32, sampleRate: 48_000,
                                               channels: 1, interleaved: false),
              let compatibleFormat = AVAudioFormat(commonFormat: .pcmFormatInt16, sampleRate: 16_000,
                                               channels: 1, interleaved: false),
              let source = AVAudioPCMBuffer(pcmFormat: sourceFormat, frameCapacity: 2048) else {
            throw DictationAudioError.unavailable
        }
        source.frameLength = 2048
        for buffer in UnsafeMutableAudioBufferListPointer(source.mutableAudioBufferList) {
            if let bytes = buffer.mData { memset(bytes, 0, Int(buffer.mDataByteSize)) }
        }
        let outputFormat = try DictationAudioConverter.analyzerFormat(preferred: sourceFormat,
                                                                     compatibleFormats: [compatibleFormat])
        guard let frame = DictationAudioCapture.copiedInput(source),
              let converted = try DictationAudioConverter(outputFormat: outputFormat).convert(frame.buffer) else {
            throw DictationAudioError.conversion
        }
        let input = AnalyzerInput(buffer: converted)
        guard input.buffer.frameLength > 0 else { throw DictationAudioError.conversion }
        delivery = Task { [weak self] in
            await Task.yield()
            guard !Task.isCancelled else { return }
            self?.onTranscript("local dictation sample")
        }
    }
    func stopCapture() { delivery?.cancel(); delivery = nil }
    func finish() async {}
    func cancel() -> Task<Void, Never> { stopCapture(); return Task {} }
}
#endif
