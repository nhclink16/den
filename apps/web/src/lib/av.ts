import type { BackgroundProcessorWrapper } from '@livekit/track-processors'
import { Track, type AudioProcessorOptions, type TrackProcessor, type VideoProcessorOptions } from 'livekit-client'
import type { CameraSettings, MicrophoneSettings } from './types'

export const defaultMicrophone: MicrophoneSettings = { gain: 1, echo_cancellation: true, noise_suppression: true, auto_gain_control: true }
export const defaultCamera: CameraSettings = { resolution: 'auto', frame_rate: 30, mirror: true, background: 'none', brightness: null, contrast: null, saturation: null }
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
const blurAssets = {
  tasksVisionFileSet: '/blur/wasm-0.10.14',
  modelAssetPath: '/blur/selfie_segmenter.tflite?v=191ac952',
}
/** Capture rates blur will try, highest first. Below the last one it gives up. */
export const blurRates = [30, 24, 15]
let supported: boolean | undefined
const mediaTransforms = globalThis as typeof globalThis & {
  MediaStreamTrackGenerator?: unknown
  MediaStreamTrackProcessor?: unknown
}
/** Background processing is deliberately limited to the modern transform API.
 * The package's canvas fallback blocks the renderer during segmentation and its
 * 0.8.0 lifecycle cannot clean up a processor cancelled while it is initializing.
 * This is the pinned 0.8.0 probe with its fallback clause removed. Keeping the
 * synchronous probe here lets the 51 KB gzip processor payload stay lazy until a
 * user actually turns blur on. The smoke covers every capability in the probe. */
export const blurSupported = () => supported ??= typeof OffscreenCanvas !== 'undefined'
  && typeof VideoFrame !== 'undefined'
  && typeof createImageBitmap !== 'undefined'
  && typeof mediaTransforms.MediaStreamTrackGenerator !== 'undefined'
  && typeof mediaTransforms.MediaStreamTrackProcessor !== 'undefined'
  && !!document.createElement('canvas').getContext('webgl2')
export const blurAccelerated = () => blurSupported()
let backgroundProcessors: Promise<typeof import('@livekit/track-processors')> | undefined
const importBackgroundProcessors = () => import('@livekit/track-processors')
const loadBackgroundProcessors = () => backgroundProcessors ??= importBackgroundProcessors()

export type CameraBackground = CameraSettings['background']
export const backgroundBlurRadius = (background: CameraBackground) => background === 'light_blur' ? 5 : 10
export const blurCaptureRate = (target: number) => blurRates.find(rate => rate <= target) ?? blurRates[0]

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
  /** A disposable clone feeds MediaStreamTrackProcessor. Stopping the real camera
   * to drain that stream would also turn the user's camera off. */
  private input?: MediaStreamTrack
  private samples: number[] = []
  private generation = 0
  /** @param target the saved capture rate blur should try to hold.
   * @param strained called when segmentation cannot hold `rate`: a number is the
   * lower rate to capture at, `null` means this machine cannot run blur at all. */
  constructor(private target: () => number, private background: () => CameraBackground, private strained: (rate: number | null) => void) {}
  get processedTrack() { return this.inner?.processedTrack }
  async init(options: VideoProcessorOptions) {
    const generation = ++this.generation
    this.source = options.track
    this.rate = blurCaptureRate(this.target())
    this.samples = []
    let BackgroundProcessor: typeof import('@livekit/track-processors')['BackgroundProcessor']
    try { ({ BackgroundProcessor } = await loadBackgroundProcessors()) } catch (error) {
      if (generation === this.generation) this.source = undefined
      throw error
    }
    if (generation !== this.generation) return
    const input = options.track.clone()
    this.input = input
    const inner = BackgroundProcessor({
      mode: 'background-blur', blurRadius: backgroundBlurRadius(this.background()), maxFps: this.rate, assetPaths: blurAssets,
      // track-processors 0.8.0's processingTimeMs adds segmentationTimeMs to
      // filterTimeMs even though filterTimeMs already spans segmentation + draw.
      // Its modern path also ignores maxFps, so this total drives a real capture
      // constraint in call.svelte.ts rather than relying on the wrapper to throttle.
      onFrameProcessed: ({ filterTimeMs }) => this.measure(filterTimeMs),
    }, this.name)
    this.inner = inner
    try {
      await inner.init({ ...options, track: input })
      if (generation !== this.generation || this.inner !== inner) {
        await this.dispose(inner, input)
        return
      }
    } catch (error) {
      if (this.inner === inner) this.inner = undefined
      if (this.input === input) this.input = undefined
      // track-processors 0.8.0 ignores destroy() while `initializing`. The modern
      // path exposes enough state to release everything it created before failing.
      await this.release(inner, input)
      throw error
    }
  }
  async setBackground(background: Exclude<CameraBackground, 'none'>) {
    await this.inner?.switchTo({ mode: 'background-blur', blurRadius: backgroundBlurRadius(background) })
  }
  /** Release the public resources exposed by 0.8.0 when its own control-stream
   * close stalls. The generated track is no longer on the sender by the time this
   * fallback returns, and destroying the transformer closes MediaPipe/WebGL. */
  private async release(inner: BackgroundProcessorWrapper, input?: MediaStreamTrack) {
    input?.stop()
    inner.processedTrack?.stop()
    inner.trackGenerator?.stop()
    inner.displayCanvas?.remove()
    await inner.transformer.destroy().catch(() => {})
  }
  private async dispose(inner: BackgroundProcessorWrapper, input?: MediaStreamTrack) {
    // Draining a processor that owns a camera track depends on a Chromium-specific
    // writableControl close. Feed it a clone instead, then end that clone first so
    // the readable stream has a standards-based reason to finish.
    input?.stop()
    let timer: ReturnType<typeof setTimeout> | undefined
    try {
      const stalled = Symbol('processor teardown stalled')
      const result = await Promise.race([
        inner.destroy(),
        new Promise<typeof stalled>(resolve => { timer = setTimeout(() => resolve(stalled), 1_000) }),
      ])
      if (result === stalled) await this.release(inner, input)
    } catch (error) {
      await this.release(inner, input)
      throw error
    } finally { clearTimeout(timer) }
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
    ++this.generation
    const inner = this.inner
    const input = this.input
    this.inner = undefined; this.input = undefined; this.source = undefined; this.samples = []
    if (!inner) return
    await this.dispose(inner, input)
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
  try {
    await blur.init({ kind: Track.Kind.Video, track, element })
    void element.play().catch(() => {})
    return element
  } catch (error) {
    element.srcObject = null
    throw error
  }
}
