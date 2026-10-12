import { test } from 'node:test'
import assert from 'node:assert/strict'
import { shellParticipants, type ShellCall, type ShellPerson } from '../apps/web/src/lib/shell-call.ts'

// In Den Shell the call view is drawn from the native engine's snapshot, so it must list people exactly as the
// web engine would: the dock's people count, the Jam and the DJ all filter on these fields.
const person = (identity: string, extra: Partial<ShellPerson> = {}): ShellPerson =>
  ({ identity, name: '', local: false, speaking: false, muted: false, quality: 'good', camera: false, screen: false, ...extra })
const snapshot = (participants: ShellPerson[]): ShellCall =>
  ({ revision: 1, phase: 'connected', channelId: 'hangout1', micSeq: 0, soundSeq: 0, micOn: true, soundOff: false, otherDevices: 0, participants })
const names: Record<string, string> = { maple: 'Maple', robin: 'Robin' }
const list = (s: ShellCall) => shellParticipants(s, (id) => names[id] || 'Someone', 'Den DJ')

test('the music bot is marked as music and keeps its own name', () => {
  const [, dj, named] = list(snapshot([person('maple:a', { local: true }), person('den-dj-hangout1'), person('den-dj-hangout1x', { name: 'Not ours' })]))
  assert.equal(dj!.music, true)
  assert.equal(dj!.name, 'Den DJ', 'falls back to the server name')
  assert.equal(named!.music, false, "another room's DJ identity is not this room's music")
  assert.equal(list(snapshot([person('den-dj-hangout1', { name: 'Jukebox' })]))[0]!.name, 'Jukebox')
})

test('people are named by account, and one account on two devices is told apart', () => {
  const out = list(snapshot([person('maple:b', { local: true }), person('maple:a'), person('robin:x', { name: 'livekit name' })]))
  assert.deepEqual(out.map((p) => [p.userId, p.name, p.device]), [['maple', 'Maple', 'This device'], ['maple', 'Maple', 'Device 1'], ['robin', 'Robin', '']])
})

test('speaking, muted and quality pass through, and streams the phone cannot show are flagged', () => {
  const [a, b, c] = list(snapshot([
    person('robin:x', { speaking: true, muted: true, quality: 'poor', camera: true }),
    person('maple:a', { camera: true, screen: true }),
    person('oak:z'),
  ]))
  assert.deepEqual([a!.speaking, a!.muted, a!.quality, a!.unseen], [true, true, 'poor', 'camera'])
  assert.equal(b!.unseen, 'screen', 'a share is the bigger thing to say')
  assert.equal(c!.unseen, undefined)
  assert.equal(a!.camera, undefined, 'no track, so the tile never tries to attach one')
  assert.deepEqual(a!.screens, [])
})
