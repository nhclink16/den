import { ScreenSharePresets, VideoPreset, type TrackPublishOptions } from 'livekit-client'

// A 720p layer keeps a full-width iPhone tile (~1080x608 px requested) off the 1080p VP8 layer, which iOS decodes in software.
// Not H.264: Chromium throttled H.264 screen-share simulcast to ~10 fps in testing.
export const firstShareLayers = [ScreenSharePresets.h360fps15, ScreenSharePresets.h720fps15]

// LiveKit republishes with these saved options after a full reconnect, so every layer must carry the new cap.
export function setShareFramerate(options: TrackPublishOptions, fps: number) {
  options.screenShareEncoding = { ...options.screenShareEncoding, maxBitrate: options.screenShareEncoding?.maxBitrate ?? 2_500_000, maxFramerate: fps }
  // Replace, never mutate: the presets are shared ScreenSharePresets instances.
  options.screenShareSimulcastLayers = options.screenShareSimulcastLayers?.map(p =>
    new VideoPreset({ width: p.width, height: p.height, aspectRatio: p.aspectRatio, maxBitrate: p.encoding.maxBitrate, maxFramerate: fps, priority: p.encoding.priority }))
}
