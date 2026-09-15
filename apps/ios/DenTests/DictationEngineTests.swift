import AVFoundation
import Foundation
import Speech
import Testing
@testable import Den

@Suite(.serialized) struct DictationEngineTests {
    @Test(arguments: DictationPCMCase.allCases)
    @MainActor func tapCopiesHardwarePCMBeforeConversion(_ sample: DictationPCMCase) async throws {
        let frame = try await Task.detached {
            let format = try #require(AVAudioFormat(commonFormat: sample.int16 ? .pcmFormatInt16 : .pcmFormatFloat32,
                                                   sampleRate: 48_000, channels: sample.stereo ? 2 : 1,
                                                   interleaved: sample.interleaved))
            let source = try #require(AVAudioPCMBuffer(pcmFormat: format, frameCapacity: 4096))
            source.frameLength = sample.empty ? 0 : 2048
            // A tap may deliver fewer frames than its allocation. Poison spare capacity so
            // copying or analyzing the unused tail cannot accidentally look like valid audio.
            for item in UnsafeMutableAudioBufferListPointer(source.mutableAudioBufferList) {
                if let data = item.mData { memset(data, 0x6b, Int(item.mDataByteSize)) }
            }
            let buffers = UnsafeMutableAudioBufferListPointer(UnsafeMutablePointer(mutating: source.audioBufferList))
            for (bufferIndex, item) in buffers.enumerated() {
                guard let data = item.mData else { continue }
                if sample.int16 {
                    let values = data.assumingMemoryBound(to: Int16.self)
                    for index in 0..<Int(item.mDataByteSize) / MemoryLayout<Int16>.size {
                        let channel = sample.interleaved ? index % Int(item.mNumberChannels) : bufferIndex
                        let frameIndex = sample.interleaved ? index / Int(item.mNumberChannels) : index
                        values[index] = sample.amplitude(frame: frameIndex, channel: channel)
                    }
                } else {
                    let values = data.assumingMemoryBound(to: Float.self)
                    for index in 0..<Int(item.mDataByteSize) / MemoryLayout<Float>.size {
                        let channel = sample.interleaved ? index % Int(item.mNumberChannels) : bufferIndex
                        let frameIndex = sample.interleaved ? index / Int(item.mNumberChannels) : index
                        values[index] = Float(sample.amplitude(frame: frameIndex, channel: channel)) / 32768
                    }
                }
            }
            let expected = pcmBytes(source)
            // The same function is invoked by the live hardware tap, not a substitute driver.
            let captured = DictationAudioCapture.copiedInput(source)
            if sample.empty {
                #expect(captured == nil || captured?.buffer.frameLength == 0, "An empty callback must not trap or manufacture samples.")
                return captured
            }
            let input = try #require(captured)
            #expect(input.buffer !== source, "The tap must own its copy after the borrowed hardware buffer returns.")
            #expect(input.buffer.format == format)
            #expect(input.buffer.frameLength == 2048)
            #expect(pcmBytes(input.buffer) == expected)
            for item in buffers {
                if let data = item.mData { memset(data, 0, Int(item.mDataByteSize)) }
            }
            #expect(pcmBytes(input.buffer) == expected, "Reusing the source buffer must not alter the captured frame.")
            return captured
        }.value

        let floatFormat = try #require(AVAudioFormat(commonFormat: .pcmFormatFloat32, sampleRate: 48_000,
                                                    channels: sample.stereo ? 2 : 1, interleaved: false))
        let intFormat = try #require(AVAudioFormat(commonFormat: .pcmFormatInt16, sampleRate: 48_000,
                                                  channels: 1, interleaved: false))
        let stereoIntFormat = try #require(AVAudioFormat(commonFormat: .pcmFormatInt16, sampleRate: 48_000,
                                                        channels: 2, interleaved: false))
        let outputFormat = try DictationAudioConverter.analyzerFormat(preferred: floatFormat,
                                                                      compatibleFormats: [floatFormat, stereoIntFormat, intFormat])
        #expect(outputFormat == intFormat, "Neither Float32 nor stereo may bypass AnalyzerInput's mono Int16 preconditions.")
        #expect(try DictationAudioConverter.analyzerFormat(preferred: stereoIntFormat,
                                                          compatibleFormats: [stereoIntFormat, intFormat]) == intFormat)
        #expect(throws: DictationAudioError.self) {
            try DictationAudioConverter.analyzerFormat(preferred: floatFormat, compatibleFormats: [floatFormat])
        }
        #expect(throws: DictationAudioError.self) {
            try DictationAudioConverter.analyzerFormat(preferred: stereoIntFormat, compatibleFormats: [stereoIntFormat])
        }
        let converter = DictationAudioConverter(outputFormat: outputFormat)
        if sample.empty {
            let empty = try #require(AVAudioPCMBuffer(pcmFormat: floatFormat, frameCapacity: 1))
            #expect(try converter.convert(empty) == nil, "An empty callback must not manufacture converted samples.")
            return
        }
        let owned = try #require(frame)
        let converted = try #require(try converter.convert(owned.buffer))
        #expect(converted.format == intFormat)
        #expect(converted.frameLength == 2048, "Same-rate normalization must preserve the frame count.")
        for item in UnsafeMutableAudioBufferListPointer(UnsafeMutablePointer(mutating: converted.audioBufferList)) {
            let data = try #require(item.mData)
            let values = data.assumingMemoryBound(to: Int16.self)
            let samples = UnsafeBufferPointer(start: values, count: Int(item.mDataByteSize) / MemoryLayout<Int16>.size)
            #expect(samples.count == Int(converted.frameLength), "Check every valid mono sample, never spare allocation capacity.")
            if sample.stereo {
                try #require(samples.count == 2048)
                let silence = samples[0..<512], left = samples[512..<1024], right = samples[1024..<1536]
                // AudioConverter.h documents format/layout-dependent gain, not fixed .5/.5 averaging.
                // Isolated channels prove neither was discarded; joint input must remain additive.
                #expect(silence.allSatisfy { $0 == 0 }, "Mixing must not manufacture sound from silence.")
                #expect(left.allSatisfy { $0 > 0 && $0 <= 4096 }, "The left channel must contribute without gain beyond its input amplitude.")
                #expect(right.allSatisfy { $0 > 0 && $0 <= 12288 }, "The right channel must contribute without gain beyond its input amplitude.")
                #expect(left.allSatisfy { abs(Int($0) - Int(samples[512])) <= 1 }, "A constant left-only segment must remain constant.")
                #expect(right.allSatisfy { abs(Int($0) - Int(samples[1024])) <= 1 }, "A constant right-only segment must remain constant.")
                #expect((0..<512).allSatisfy {
                    abs(Int(samples[1536 + $0]) - Int(samples[512 + $0]) - Int(samples[1024 + $0])) <= 2
                }, "Both channels together must equal their isolated contributions within two quantization steps. Case=\(sample.rawValue), left=\(samples[512]), right=\(samples[1024]), both=\(samples[1536]).")
            } else {
                let mismatches = samples.enumerated().filter { abs(Int($0.element) - 8192) > 1 }
                let firstMismatches = mismatches.prefix(12).map { "\($0.offset):\($0.element)" }
                #expect(samples.allSatisfy { abs(Int($0) - 8192) <= 1 },
                        "Mono must preserve quarter-scale. Case=\(sample.rawValue), frames=\(converted.frameLength), samples=\(samples.count), min=\(String(describing: samples.min())), max=\(String(describing: samples.max())), first=\(Array(samples.prefix(16))), mismatchCount=\(mismatches.count), firstMismatches=\(firstMismatches).")
            }
        }
        // Exercise the real framework constructor only after the same conversion used in production.
        let analyzed = AnalyzerInput(buffer: converted).buffer
        #expect(analyzed.frameLength == converted.frameLength)
        #expect(analyzed.format == intFormat)
        #expect(pcmBytes(analyzed) == pcmBytes(converted))
    }

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
        let url = URL(fileURLWithPath: "/tmp/den-dictation-test.caf")
        #expect(throws: DictationAudioError.self) {
            try DictationPlatform.legacyRequest(url: url, supportsOnDeviceRecognition: false, punctuation: true)
        }
        for punctuation in [false, true] {
            let request = try DictationPlatform.legacyRequest(url: url, supportsOnDeviceRecognition: true, punctuation: punctuation)
            #expect(request.url == url, "The fallback must read the completed local recording.")
            #expect(request.requiresOnDeviceRecognition, "A fallback request must never send audio to a service.")
            #expect(!request.shouldReportPartialResults, "Record-then-transcribe publishes only final text.")
            #expect(request.addsPunctuation == punctuation)
        }
    }

    @Test @MainActor func recordingKeepsBothSidesOfALongPauseAndRemovesItsPrivateFile() throws {
        let format = try #require(AVAudioFormat(commonFormat: .pcmFormatInt16, sampleRate: 48_000,
                                              channels: 1, interleaved: false))
        let recording = try DictationRecording(format: format)
        defer { recording.remove() }
        let directory = recording.directory
        let samplesPerSecond = 48_000
        for (amplitude, seconds) in [(Int16(8192), 1), (Int16(0), 3), (Int16(-16384), 1)] {
            let buffer = try #require(AVAudioPCMBuffer(pcmFormat: format, frameCapacity: AVAudioFrameCount(samplesPerSecond * seconds)))
            buffer.frameLength = buffer.frameCapacity
            let samples = try #require(buffer.int16ChannelData?[0])
            for index in 0..<Int(buffer.frameLength) { samples[index] = amplitude }
            try recording.append(buffer)
            if amplitude == 0 { #expect(recording.audioLevel == 0, "Silence must produce a quiet meter, not a fabricated animation.") }
            else { #expect(recording.audioLevel > 0 && recording.audioLevel <= 1, "The meter must reflect the written audio.") }
        }
        #expect(recording.duration == 5, "The recorded timeline includes the full three-second pause.")
        let file = try recording.openForReading()
        #expect(file.length == 240_000, "Stopping must retain both spoken regions and the intervening silence.")
        #expect(file.processingFormat.commonFormat == .pcmFormatInt16)
        #expect(file.processingFormat.channelCount == 1, "File transcription must retain iOS 27's safe AnalyzerInput format.")
        let read = try #require(AVAudioPCMBuffer(pcmFormat: file.processingFormat, frameCapacity: 4096))
        var samples: [Int16] = []
        while file.framePosition < file.length {
            try file.read(into: read)
            try #require(read.frameLength > 0, "Reading must progress until the full file has been consumed.")
            let channel = try #require(read.int16ChannelData?[0])
            samples.append(contentsOf: UnsafeBufferPointer(start: channel, count: Int(read.frameLength)))
        }
        try #require(samples.count == 240_000, "Check every recorded sample, including a partial final read.")
        #expect((0..<48_000).allSatisfy { samples[$0] == 8192 })
        #expect((48_000..<192_000).allSatisfy { samples[$0] == 0 })
        let tailMismatches = (192_000..<240_000).filter { samples[$0] != -16384 }
        #expect(tailMismatches.isEmpty, "Final segment mismatches: \(tailMismatches.count); first: \(tailMismatches.prefix(8).map { "\($0):\(samples[$0])" })")
        recording.remove()
        #expect(!FileManager.default.fileExists(atPath: directory.path), "Cancellation/completion must leave no audio or directory behind.")
        recording.remove() // Cleanup must also tolerate a cancellation racing completion.
    }

    @Test(.enabled(if: ProcessInfo.processInfo.environment["DEN_TEST_DICTATION_PERMISSIONS"] == "1",
                   "Opt-in system permission integration: may show the microphone prompt; never captures audio."))
    @MainActor func platformPermissionCallbacksResumeOnOwningActor() async {
        var ownershipChecks = 0
        let speechBefore = SFSpeechRecognizer.authorizationStatus()
        let authorization = await DictationPlatform.authorize(isCurrent: {
            ownershipChecks += 1
            return true
        })
        #expect(AVAudioApplication.shared.recordPermission == .granted)
        #expect(SFSpeechRecognizer.authorizationStatus() == speechBefore, "Local dictation must not request or change legacy speech authorization.")
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
        #expect(received.isEmpty, "Recording must leave the draft untouched.")
        fixture.controller.invalidateContext()
        #expect(!first.capturing, "Context invalidation closes capture synchronously.")
        await fixture.controller.start { received.append($0) }
        let second = try #require(fixture.drivers.last)
        #expect(fixture.discoveryCount == 2, "Starting a new session reuses discovered languages; the driver rechecks readiness.")
        second.emit("second")
        first.emit("late result from first")
        first.end(false)
        #expect(received.isEmpty, "Canceled and still-recording sessions must never publish text.")
        #expect(fixture.controller.state == .listening, "Old completion must not stop the new session.")
        #expect(second.capturing)

        fixture.callActive = true
        second.emit("late result after call became active")
        #expect(!second.capturing)
        #expect(!fixture.controller.isActive)
        #expect(received.isEmpty, "A call starting discards the pending recording, including buffered recognition.")
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
        #expect(received.isEmpty)
        fixture.controller.stop()
        #expect(!first.capturing, "stop() must remove the hardware tap before returning.")
        #expect(fixture.controller.state == .finishing)
        fixture.controller.stop() // An outside tap and focus loss can both report the same stop.
        #expect(fixture.controller.state == .finishing, "A repeated stop must preserve pending final words.")
        await wait { first.finishWaiter != nil }
        first.emit("keep my last word")
        try await Task.sleep(for: .milliseconds(2200))
        #expect(fixture.controller.state == .finishing, "File transcription must not be cut off by the old two-second streaming deadline.")
        #expect(received.isEmpty, "Even a final callback stays buffered until the whole file finishes.")
        first.releaseFinish()
        await wait { !fixture.controller.isActive }
        #expect(received == ["keep my last word"], "Publish the entire recording exactly once, not intermediate revisions.")

        await fixture.controller.start { received.append($0) }
        let second = try #require(fixture.drivers.last)
        second.emit("editable text")
        fixture.controller.stop()
        await wait { second.finishWaiter != nil }
        fixture.controller.invalidateContext() // A caret move or manual edit owns the text now.
        second.emit("late final must not overwrite the edit")
        second.releaseFinish()
        #expect(received == ["keep my last word"], "Canceling a pending transcription must not insert any of that recording.")
        #expect(!fixture.controller.isActive)

        await fixture.controller.start { received.append($0) }
        let third = try #require(fixture.drivers.last)
        fixture.controller.stop()
        await wait { third.finishWaiter != nil }
        third.emit("incomplete result")
        try await Task.sleep(for: .milliseconds(4200))
        #expect(!fixture.controller.isActive, "A recognizer that never finishes must release the session within the deadline.")
        #expect(fixture.controller.error?.contains("too long") == true)
        third.emit("too late after finalization deadline")
        #expect(received == ["keep my last word"], "A timeout must discard incomplete text and all late callbacks.")
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

enum DictationPCMCase: String, CaseIterable, Sendable {
    case float32MonoPlanar, float32StereoPlanar, float32MonoInterleaved, float32StereoInterleaved
    case int16MonoPlanar, int16StereoPlanar, int16MonoInterleaved, int16StereoInterleaved
    case emptyFloat32MonoPlanar
    var int16: Bool { rawValue.hasPrefix("int16") }
    var stereo: Bool { rawValue.contains("Stereo") }
    var interleaved: Bool { rawValue.hasSuffix("Interleaved") }
    var empty: Bool { self == .emptyFloat32MonoPlanar }

    func amplitude(frame: Int, channel: Int) -> Int16 {
        guard stereo else { return 8192 }
        switch frame / 512 {
        case 0: return 0
        case 1: return channel == 0 ? 4096 : 0
        case 2: return channel == 1 ? 12288 : 0
        default: return channel == 0 ? 4096 : 12288
        }
    }
}

private func pcmBytes(_ buffer: AVAudioPCMBuffer) -> [Data] {
    UnsafeMutableAudioBufferListPointer(UnsafeMutablePointer(mutating: buffer.audioBufferList)).map {
        guard let data = $0.mData, $0.mDataByteSize > 0 else { return Data() }
        return Data(bytes: data, count: Int($0.mDataByteSize))
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
        }, finishTimeout: .milliseconds(4000)))
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
