import { Track, type AudioProcessorOptions, type TrackProcessor } from 'livekit-client'
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
export function cameraConstraints(p: CameraSettings, track: MediaStreamTrack): MediaTrackConstraints {
  const caps = capabilities(track)
  const height = p.resolution === '720p' ? 720 : p.resolution === '1080p' ? 1080 : 0
  const width = height * 16 / 9
  const result: MediaTrackConstraints = {}
  // Exact constraints make rejected combinations visible instead of pretending they applied.
  if (height && supports(caps.height, height) && supports(caps.width, width)) {
    result.width = { exact: width }; result.height = { exact: height }
  }
  if (supports(caps.frameRate, p.frame_rate)) result.frameRate = { exact: p.frame_rate }
  const picture: Partial<Record<PictureControl, number>> = {}
  for (const key of ['brightness', 'contrast', 'saturation'] as const) {
    const value = p[key], range = caps[key]
    if (range && value != null && supports(range, value)) picture[key] = value
  }
  if (Object.keys(picture).length) result.advanced = [picture as MediaTrackConstraintSet]
  return result
}

/** Microphone -> gain -> destination. Soundboards can mix into gain;
 * voice mods can process between source and gain. Never connect to speakers. */
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
    this.input.connect(this.gain); this.gain.connect(this.destination)
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
