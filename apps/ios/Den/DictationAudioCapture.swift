import AVFoundation
import Speech
import Synchronization
import UIKit

enum DictationAudioError: Error { case unavailable, overflow, conversion }

/// The tap makes an owned copy before publishing this frame and never mutates it again.
/// Its single consumer only reads the buffer, including AVAudioConverter's synchronous callback.
/// AVAudioPCMBuffer is not Sendable; this wrapper transfers that narrowly scoped ownership.
struct DictationPCMFrame: @unchecked Sendable {
    let buffer: AVAudioPCMBuffer
    fileprivate init(_ buffer: AVAudioPCMBuffer) { self.buffer = buffer }
}

/// The sole owner of dictation hardware. stop() never schedules audio-session work.
@MainActor final class DictationAudioCapture {
    private let engine = AVAudioEngine()
    private var continuation: AsyncThrowingStream<DictationPCMFrame, Error>.Continuation?
    private var installedTap = false
    private var ownsAudioSession = false
    private var observers: [NSObjectProtocol] = []

    func start(onInterrupted: @escaping @MainActor () -> Void) throws -> AsyncThrowingStream<DictationPCMFrame, Error> {
        guard !installedTap else { throw DictationAudioError.unavailable }
        try Self.requireForeground()
        let session = AVAudioSession.sharedInstance()
        try session.setCategory(.record, mode: .measurement)
        try session.setActive(true)
        ownsAudioSession = true
        do {
            let input = engine.inputNode
            let format = input.outputFormat(forBus: 0)
            guard format.sampleRate > 0, format.channelCount > 0 else { throw DictationAudioError.unavailable }
            let pair = AsyncThrowingStream<DictationPCMFrame, Error>.makeStream(bufferingPolicy: .bufferingOldest(32))
            continuation = pair.continuation
            input.installTap(onBus: 0, bufferSize: 2048, format: format) { @Sendable buffer, _ in
                // Tap buffers are borrowed. Copy before crossing into an async consumer.
                guard buffer.frameLength > 0 else { return }
                guard let input = Self.copiedInput(buffer) else {
                    pair.continuation.finish(throwing: DictationAudioError.unavailable); return
                }
                if case .dropped = pair.continuation.yield(input) {
                    pair.continuation.finish(throwing: DictationAudioError.overflow)
                }
            }
            installedTap = true
            engine.prepare(); try engine.start()
            for name in [UIApplication.didEnterBackgroundNotification, AVAudioSession.interruptionNotification,
                         AVAudioSession.mediaServicesWereResetNotification, .AVAudioEngineConfigurationChange] {
                observers.append(NotificationCenter.default.addObserver(forName: name, object: nil, queue: .main) { _ in
                    MainActor.assumeIsolated { onInterrupted() }
                })
            }
            return pair.stream
        } catch { stop(); throw error }
    }

    static func requireForeground() throws {
        guard UIApplication.shared.applicationState == .active else { throw DictationAudioError.unavailable }
    }

    /// Microphone authorization can resume before its system sheet finishes dismissing.
    static func waitForForeground() async throws {
        for _ in 0..<100 {
            try Task.checkCancellation()
            if UIApplication.shared.applicationState == .active { return }
            guard UIApplication.shared.applicationState == .inactive else { throw CancellationError() }
            try await Task.sleep(for: .milliseconds(20))
        }
        throw CancellationError()
    }

    func stop() {
        observers.forEach(NotificationCenter.default.removeObserver); observers.removeAll()
        if installedTap { engine.inputNode.removeTap(onBus: 0); installedTap = false }
        engine.stop()
        continuation?.finish(); continuation = nil
        if ownsAudioSession {
            ownsAudioSession = false
            try? AVAudioSession.sharedInstance().setActive(false, options: .notifyOthersOnDeactivation)
        }
    }

    nonisolated static func copiedInput(_ source: AVAudioPCMBuffer) -> DictationPCMFrame? {
        guard source.frameLength > 0, let buffer = copy(source) else { return nil }
        return DictationPCMFrame(buffer)
    }

    private nonisolated static func copy(_ source: AVAudioPCMBuffer) -> AVAudioPCMBuffer? {
        guard let target = AVAudioPCMBuffer(pcmFormat: source.format, frameCapacity: source.frameLength) else { return nil }
        target.frameLength = source.frameLength
        // Read only initialized frames; the mutable list advertises frameCapacity instead.
        let input = UnsafeMutableAudioBufferListPointer(UnsafeMutablePointer(mutating: source.audioBufferList))
        let output = UnsafeMutableAudioBufferListPointer(target.mutableAudioBufferList)
        guard input.count == output.count else { return nil }
        for index in input.indices {
            guard let from = input[index].mData, let to = output[index].mData,
                  output[index].mDataByteSize >= input[index].mDataByteSize else { return nil }
            memcpy(to, from, Int(input[index].mDataByteSize))
        }
        return target
    }
}

/// Used only by one ordered input consumer, never concurrently with the audio tap.
@MainActor final class DictationAudioConverter {
    private var converter: AVAudioConverter?
    private let outputFormat: AVAudioFormat
    init(outputFormat: AVAudioFormat) { self.outputFormat = outputFormat }

    /// iOS 27 AnalyzerInput requires mono Int16. Never manufacture a rate/channel layout
    /// that the selected module has not advertised as compatible.
    static func analyzerFormat(preferred: AVAudioFormat?, compatibleFormats: [AVAudioFormat]) throws -> AVAudioFormat {
        func usable(_ format: AVAudioFormat) -> Bool {
            format.commonFormat == .pcmFormatInt16 && format.sampleRate.isFinite &&
                format.sampleRate > 0 && format.channelCount == 1
        }
        if let preferred, usable(preferred) { return preferred }
        guard let format = compatibleFormats.first(where: usable) else { throw DictationAudioError.unavailable }
        return format
    }

    func convert(_ buffer: AVAudioPCMBuffer) throws -> AVAudioPCMBuffer? {
        guard buffer.frameLength > 0 else { return nil }
        if buffer.format == outputFormat { return buffer }
        if converter?.inputFormat != buffer.format {
            converter = AVAudioConverter(from: buffer.format, to: outputFormat)
            converter?.downmix = buffer.format.channelCount > outputFormat.channelCount
        }
        guard let converter, buffer.format.sampleRate > 0 else { throw DictationAudioError.conversion }
        let capacity = AVAudioFrameCount(ceil(Double(buffer.frameLength) * outputFormat.sampleRate / buffer.format.sampleRate) + 64)
        guard let output = AVAudioPCMBuffer(pcmFormat: outputFormat, frameCapacity: capacity) else { throw DictationAudioError.conversion }
        let provided = Mutex(false)
        let source = DictationPCMFrame(buffer)
        var error: NSError?
        let status = converter.convert(to: output, error: &error) { @Sendable _, inputStatus in
            let first = provided.withLock { value in
                guard !value else { return false }
                value = true; return true
            }
            guard first else { inputStatus.pointee = .noDataNow; return nil }
            inputStatus.pointee = .haveData; return source.buffer
        }
        guard status != .error, error == nil else { throw DictationAudioError.conversion }
        return output.frameLength > 0 ? output : nil
    }
}
