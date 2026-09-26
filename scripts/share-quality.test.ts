import { test } from 'node:test'
import assert from 'node:assert/strict'
import type { TrackPublishOptions } from 'livekit-client'
import { firstShareLayers, setShareFramerate } from '../apps/web/src/lib/share-quality.ts'

test('Smooth survives a republish on every screen-share layer, and Sharp restores 15', () => {
  const options: TrackPublishOptions = { screenShareSimulcastLayers: firstShareLayers }
  setShareFramerate(options, 30)
  assert.equal(options.screenShareEncoding?.maxFramerate, 30)
  assert.equal(options.screenShareEncoding?.maxBitrate, 2_500_000)
  assert.deepEqual(options.screenShareSimulcastLayers?.map(p => [p.height, p.encoding.maxBitrate, p.encoding.maxFramerate]), [[360, 400_000, 30], [720, 1_500_000, 30]])
  // The shared presets the next share publishes with stay at 15.
  assert.deepEqual(firstShareLayers.map(p => p.encoding.maxFramerate), [15, 15])
  setShareFramerate(options, 15)
  assert.deepEqual(options.screenShareSimulcastLayers?.map(p => p.encoding.maxFramerate), [15, 15])
})

test('an extra share keeps its bitrate cap and gains no layers', () => {
  const options: TrackPublishOptions = { screenShareEncoding: { maxBitrate: 750_000, maxFramerate: 15 } }
  setShareFramerate(options, 30)
  assert.deepEqual(options.screenShareEncoding, { maxBitrate: 750_000, maxFramerate: 30 })
  assert.equal(options.screenShareSimulcastLayers, undefined)
})
