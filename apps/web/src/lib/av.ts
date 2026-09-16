import { BackgroundProcessor, supportsBackgroundProcessors, supportsModernBackgroundProcessors, type BackgroundProcessorWrapper } from '@livekit/track-processors'
import { Track, type AudioProcessorOptions, type TrackProcessor, type VideoProcessorOptions } from 'livekit-client'
import type { CameraSettings, MicrophoneSettings } from './types'

export const defaultMicrophone: MicrophoneSettings = { gain: 1, echo_cancellation: true, noise_suppression: true, auto_gain_control: true }
export const defaultCamera: CameraSettings = { resolution: 'auto', frame_rate: 30, mirror: true, brightness: null, contrast: null, saturation: null }
export type PictureControl = 'brightness' | 'contrast' | 'saturation'
export type CameraCapabilities = MediaTrackCapabilities & Partial<Record<PictureControl, { min: number; max: number; step?: number }>>
export const deviceId = (track?: MediaStreamTrack | null) => track?.getSettings().deviceId || 'default'
export const capabilities = (track: MediaStreamTrack): CameraCapabilities => track.getCapabilities?.() || {}
export const supports = (range: { min?: number; max?: number } | undefined, value: number) => !!range && value >= (range.min ?? 0) && value <= (range.max ?? 0)
export function microphoneConstraints(p: MicrophoneSettings, track?: MediaStreamTrack): { echoCancellation?: ConstrainBoolean; noiseSuppression?: ConstrainBoolean; autoGainControl?: ConstrainBoolean } {
  const caps = track?.getCapabilities?.()
  const result: { echoCancellation?: ConstrainBoolean; noiseSuppression?: ConstrainBoolean; autoGainControl?: ConstrainBoolean } = {}
  for (const [key, value] of [['echoCancellation', p.echo_cancellation], ['noiseSuppression', p.noise_suppression], ['autoGainControl', p.auto_gain_control]] as const) {
    if (!caps || caps[key]?.includes(value)) result[key] = { exact: value }
  }
  return result
}
/** `cap` is the rate background blur measured this machine can actually segment.
 * It is Den's own limit rather than a saved choice, so it asks for a maximum
 * instead of an exact rate: the saved preference is left alone and comes back
 * untouched when blur is turned off. */
export function cameraConstraints(p: CameraSettings, track: MediaStreamTrack, cap?: number): MediaTrackConstraints {
  const caps = capabilities(track)
  const height = p.resolution === '720p' ? 720 : p.resolution === '1080p' ? 1080 : 0
  const width = height * 16 / 9
  const result: MediaTrackConstraints = {}
  // Exact constraints make rejected combinations visible instead of pretending they applied.
  if (height && supports(caps.height, height) && supports(caps.width, width)) {
    result.width = { exact: width }; result.height = { exact: height }
  }
  if (cap && cap < p.frame_rate) result.frameRate = { max: cap }
  else if (supports(caps.frameRate, p.frame_rate)) result.frameRate = { exact: p.frame_rate }
  const picture: Partial<Record<PictureControl, number>> = {}
  for (const key of ['brightness', 'contrast', 'saturation'] as const) {
    const value = p[key], range = caps[key]
    if (range && value != null && supports(range, value)) picture[key] = value
  }
  if (Object.keys(picture).length) result.advanced = [picture as MediaTrackConstraintSet]
  return result
}

/** Microphone -> gain -> destination. Never connect to speakers. The two places a
 * future feature hooks into this chain are marked inside `init`: a mixing point on
 * the gain node for soundboards, and an insertion point between source and gain
 * for voice mods. */
export class MicrophoneGain implements TrackProcessor<Track.Kind.Audio, AudioProcessorOptions> {
  name = 'den-microphone-gain'
  processedTrack?: MediaStreamTrack
  source?: MediaStreamTrack
  private context?: AudioContext
  private input?: MediaStreamAudioSourceNode
  private gain?: GainNode
  private destination?: MediaStreamAudioDestinationNode
  constructor(private preferences: (id: string) => MicrophoneSettings) {}
  async init({ track, audioContext }: AudioProcessorOptions) {
    this.context = audioContext || this.context
    if (!this.context) throw Error('Microphone processing needs an audio context.')
    await this.context.resume()
    this.source = track
    await track.applyConstraints(microphoneConstraints(this.preferences(deviceId(track)), track))
    this.input = this.context.createMediaStreamSource(new MediaStream([track]))
    this.gain = this.context.createGain()
    this.gain.gain.value = this.preferences(deviceId(track)).gain
    this.destination = this.context.createMediaStreamDestination()
    // VOICE MODS - insertion point. An effect (pitch shift, radio filter, whatever)
    // belongs between `input` and `gain`. Build its nodes as fields on this class so
    // `destroy` can disconnect them, then connect input -> effect -> gain in place of
    // the line below. Keep `gain` last so the user's input-gain slider still has the
    // final word, and keep the effect off the Settings meter's path by leaving the
    // meter where it is: connected after gain, on `processedTrack`.
    this.input.connect(this.gain)
    // SOUNDBOARDS - mixing point. `gain` is where the outgoing mix is assembled, so
    // an extra source (a decoded clip, a second capture) connects into `gain` and
    // rides out with the voice. Give each clip its own gain node for its own level
    // rather than touching this one. It must not also connect to `context.destination`:
    // that is the speaker path, and a clip on both is heard twice and recaptured by
    // an open microphone.
    this.gain.connect(this.destination)
    this.processedTrack = this.destination.stream.getAudioTracks()[0]
  }
  setGain(value: number) { if (this.gain) this.gain.gain.value = value }
  async update() {
    if (this.gain) this.gain.gain.value = this.preferences(deviceId(this.source)).gain
    if (this.source?.readyState === 'live') await this.source.applyConstraints(microphoneConstraints(this.preferences(deviceId(this.source)), this.source))
  }
  async restart(options: AudioProcessorOptions) { await this.destroy(); await this.init(options) }
  async destroy() {
    this.input?.disconnect(); this.gain?.disconnect(); this.destination?.disconnect()
    this.processedTrack?.stop(); this.processedTrack = undefined; this.source = undefined
  }
}

// MediaPipe's wasm and model are served by Den itself. The package defaults to
// fetching them from jsdelivr and storage.googleapis.com, which a self-hosted app
// has no business doing. scripts/blur-assets.mjs stages the wasm into public/blur.
const blurAssets = { tasksVisionFileSet: '/blur/wasm', modelAssetPath: '/blur/selfie_segmenter.tflite' }
/** Capture rates blur will try, highest first. Below the last one it gives up. */
export const blurRates = [30, 24, 15]
let supported: boolean | undefined
/** The package's own probe rather than a copy of it: insertable streams or the
 * canvas fallback, plus WebGL2, OffscreenCanvas and VideoFrame for the segmenter.
 * Safari and the Tauri WebView land on the fallback or fail outright, and have to
 * get a disabled control with a reason, not a call that breaks when blur goes on. */
export const blurSupported = () => supported ??= supportsBackgroundProcessors()
/** False means the canvas fallback. It works, but it costs noticeably more. */
export const blurAccelerated = () => supportsModernBackgroundProcessors()

/** Background blur for a camera track.
 *
 * This wraps LiveKit's MediaPipe processor for a reason beyond convenience. Once a
 * processor runs, `LocalVideoTrack.mediaStreamTrack` is a generator or canvas track
 * with no device, no capabilities and no meaningful settings. Everything that reads
 * capabilities, applies constraints or keys saved preferences by device ID has to
 * read `source` instead, which is why it is public here: the SDK exposes only an
 * internal settings accessor for it. */
export class CameraBlur implements TrackProcessor<Track.Kind.Video, VideoProcessorOptions> {
  name = 'den-background-blur'
  /** The real camera behind the blur. Capabilities, constraints, `getSettings` and
   * the device ID that keys saved camera preferences all belong to this track. */
  source?: MediaStreamTrack
  /** The capture rate blur is asking for, stepped down under strain. */
  rate = blurRates[0]
  private inner?: BackgroundProcessorWrapper
  private samples: number[] = []
  /** @param target the saved capture rate blur should try to hold.
   * @param strained called when segmentation cannot hold `rate`: a number is the
   * lower rate to capture at, `null` means this machine cannot run blur at all. */
  constructor(private target: () => number, private strained: (rate: number | null) => void) {}
  get processedTrack() { return this.inner?.processedTrack }
  async init(options: VideoProcessorOptions) {
    this.source = options.track
    this.rate = blurRates.find(rate => rate <= this.target()) ?? blurRates[0]
    this.samples = []
    this.inner = BackgroundProcessor({
      mode: 'background-blur', maxFps: this.rate, assetPaths: blurAssets,
      onFrameProcessed: ({ processingTimeMs }) => this.measure(processingTimeMs),
    }, this.name)
    await this.inner.init(options)
  }
  /** The package times its own frames and then only logs the estimate, so the
   * fallback is Den's to write. Over a window of frames: if segmentation costs more
   * than the frame budget, step the capture rate down rather than dropping blur, and
   * give up only when the slowest step still cannot be held. It never steps back up
   * on its own; a camera that oscillates between two rates is worse than one that
   * settles on the lower. */
  private measure(ms: number) {
    this.samples.push(ms)
    if (this.samples.length < 60) return
    const mean = this.samples.reduce((sum, n) => sum + n, 0) / this.samples.length
    this.samples = []
    if (mean <= 1000 / this.rate) return
    const next = blurRates[blurRates.indexOf(this.rate) + 1]
    if (next) this.rate = next
    this.strained(next ?? null)
  }
  /** The SDK calls this after a device change, already holding the new camera
   * track. A new camera gets a new segmenter and a fresh rate ladder: what the old
   * camera measured says nothing about this one. */
  async restart(options: VideoProcessorOptions) { await this.destroy(); await this.init(options) }
  async destroy() {
    const inner = this.inner
    this.inner = undefined; this.source = undefined; this.samples = []
    await inner?.destroy()
  }
}

/** Start blur on a track Den captured itself. `LocalVideoTrack.setProcessor` builds
 * the input element the processor renders from, and a bare `MediaStreamTrack` has no
 * such owner, so Settings' private camera has to build the same thing. It must be a
 * playing `<video>` or the wrapper throws. Returns it so the caller can release it. */
export async function startPreviewBlur(blur: CameraBlur, track: MediaStreamTrack) {
  const element = document.createElement('video')
  element.muted = true; element.playsInline = true
  element.srcObject = new MediaStream([track])
  await blur.init({ kind: Track.Kind.Video, track, element })
  void element.play().catch(() => { /* The processor's render loop retries play itself. */ })
  return element
}
