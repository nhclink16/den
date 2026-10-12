#if DEBUG
import Foundation
import LiveKit
import OSLog

// Test hooks for the simulator call test (launch with `-DenCallTestTone YES`). Debug builds only: none of this
// is compiled into Release.
//
// The simulator records the Mac's real microphone, which can't be fed a known sound. So the mic's audio is
// replaced by a quiet 150 Hz hum, the pitch of WebKit's fake mic, inside LiveKit's capture step. LiveKit only
// runs that step while it is really capturing, so a far end that hears the hum proves the mic path is live.
// The hum is pulsed (0.4 s of every second): a steady tone is learnt as background noise and suppressed.
// The playback side logs, every 2 s, how much audio LiveKit actually rendered and its level at 440 Hz (the
// test robot's tone), as `playout` lines under the `test` category.

enum CallTestHooks {
    static func install() {
        guard UserDefaults.standard.bool(forKey: "DenCallTestTone") else { return }
        AudioManager.shared.capturePostProcessingDelegate = TestTone()
        AudioManager.shared.renderPreProcessingDelegate = PlayoutMeter()
        Logger(subsystem: "app.denchat.shell", category: "test").info("call test tone on")
    }
}

/// WebRTC's audio buffers hold floats in the 16-bit range.
private let fullScale: Float = 32767

private final class TestTone: AudioCustomProcessingDelegate, @unchecked Sendable {
    // Only touched on the audio capture thread.
    private var phase = 0.0
    private var rate = 48_000.0
    private var position = 0

    func audioProcessingInitialize(sampleRate sampleRateHz: Int, channels: Int) { rate = Double(sampleRateHz) }

    func audioProcessingProcess(audioBuffer: LKAudioBuffer) {
        let step = 2 * Double.pi * 150 / rate
        let second = Int(rate)
        var end = phase
        for channel in 0 ..< audioBuffer.channels {
            let samples = audioBuffer.rawBuffer(forChannel: channel)
            var p = phase
            // Replace, not add: the test sends a known sound and nothing of the room.
            for i in 0 ..< audioBuffer.frames {
                let on = (position + i) % second < second * 2 / 5
                samples[i] = on ? Float(sin(p)) * 0.1 * fullScale : 0
                p += step
            }
            end = p
        }
        position = (position + audioBuffer.frames) % second
        phase = end.truncatingRemainder(dividingBy: 2 * .pi)
    }

    func audioProcessingRelease() {}
}

private final class PlayoutMeter: AudioCustomProcessingDelegate, @unchecked Sendable {
    // Only touched on the audio render thread.
    private let log = Logger(subsystem: "app.denchat.shell", category: "test")
    private var rate = 48_000.0
    private var started = Date()
    private var frames = 0
    private var energy = 0.0
    // Goertzel filter at 440 Hz over the window.
    private var s1 = 0.0, s2 = 0.0

    func audioProcessingInitialize(sampleRate sampleRateHz: Int, channels: Int) { rate = Double(sampleRateHz) }

    func audioProcessingProcess(audioBuffer: LKAudioBuffer) {
        let samples = audioBuffer.rawBuffer(forChannel: 0)
        let coefficient = 2 * cos(2 * Double.pi * 440 / rate)
        for i in 0 ..< audioBuffer.frames {
            let x = Double(samples[i] / fullScale)
            energy += x * x
            let s0 = x + coefficient * s1 - s2
            s2 = s1; s1 = s0
        }
        frames += audioBuffer.frames
        let elapsed = Date().timeIntervalSince(started)
        guard elapsed >= 2 else { return }
        let power = max(s1 * s1 + s2 * s2 - coefficient * s1 * s2, 0)
        // Amplitude of the 440 Hz part, as dBFS of a sine.
        let tone = 20 * log10(max(2 * sqrt(power) / Double(max(frames, 1)) / sqrt(2), 1e-9))
        let rms = 20 * log10(max(sqrt(energy / Double(max(frames, 1))), 1e-9))
        let perSecond = Int(Double(frames) / elapsed)
        log.info("playout samplesPerSec=\(perSecond, privacy: .public) rmsDb=\(rms, format: .fixed(precision: 1), privacy: .public) tone440Db=\(tone, format: .fixed(precision: 1), privacy: .public)")
        started = Date(); frames = 0; energy = 0; s1 = 0; s2 = 0
    }

    func audioProcessingRelease() {}
}
#endif
