import { test } from 'node:test'
import assert from 'node:assert/strict'
import { anchor } from '../apps/web/src/lib/jam-clock.ts'
import { liveStatus } from '../apps/web/src/lib/status.ts'

test('a cached Jam sample does not pull the clock backwards', () => {
  const first = anchor(null, { track: 'a', sampled_at: 1_000, progress_ms: 60_000 }, 50_000)
  // Three seconds later the same cached sample arrives again: keep the running clock.
  assert.equal(anchor(first, { track: 'a', sampled_at: 1_000, progress_ms: 60_000 }, 53_000), first)
  // A fresh sample taken 5 s after the first lands 5 s after it on our clock, not at "now".
  const next = anchor(first, { track: 'a', sampled_at: 6_000, progress_ms: 65_000 }, 58_000)
  assert.deepEqual(next, { track: 'a', sampled: 6_000, at: 55_000, ms: 65_000 })
  // A new track starts over from now.
  assert.deepEqual(anchor(next, { track: 'b', sampled_at: 7_000, progress_ms: 0 }, 59_000), { track: 'b', sampled: 7_000, at: 59_000, ms: 0 })
})

test('statuses past their expiry are not shown', () => {
  const now = 2_000_000_000_000
  assert.equal(liveStatus({ emoji: '🎮', text: 'raid', expires_at: now / 1000 - 1 }, now), null)
  assert.deepEqual(liveStatus({ emoji: '🎮', text: 'raid', expires_at: now / 1000 + 60 }, now)?.text, 'raid')
  assert.equal(liveStatus({ emoji: null, text: null, expires_at: null }, now), null)
  assert.equal(liveStatus({ emoji: null, text: 'no expiry', expires_at: null }, now)?.text, 'no expiry')
})
