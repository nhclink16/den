import { test } from 'node:test'
import assert from 'node:assert/strict'
import { ReadState } from '../apps/web/src/lib/read-state.ts'
import type { ChannelReadState } from '../apps/web/src/lib/types.ts'

const state = (channel_id: string, unread_count: number, last_read_id = ''): ChannelReadState =>
  ({ channel_id, last_read_id, unread_count, mention_count: 0, notification_count: 0 })

/// A promise the test releases by hand, standing in for a response held open.
function held<T>() {
  let release!: (v: T) => void
  let fail!: (e: unknown) => void
  const promise = new Promise<T>((res, rej) => { release = res; fail = rej })
  return { promise, release, fail }
}

/// The real owner over a real map, the way the Store holds it.
function owner() {
  let counts = new Map<string, ChannelReadState>()
  const reads = new ReadState({ get: () => counts, set: (v) => { counts = v } })
  return { reads, map: () => counts, count: (key: string) => counts.get(key)?.unread_count }
}

test('a socket update beats the stale response it overtook', async () => {
  const { reads, count } = owner()
  const response = held<ChannelReadState>()
  const request = reads.serialize('c1', async (at) => reads.applyIf(await response.promise, reads.epoch, at))
  reads.apply(state('c1', 1))
  response.release(state('c1', 0))
  assert.equal(await request, false, 'the stale body was applied over the socket update')
  assert.equal(count('c1'), 1, 'the newer count was lost')
})

test('a delayed response still applies when nothing newer landed', async () => {
  // The control: same shape, no interleaved update. Without it, "never apply
  // anything" would pass the suite.
  const { reads, count } = owner()
  const response = held<ChannelReadState>()
  const request = reads.serialize('c1', async (at) => reads.applyIf(await response.promise, reads.epoch, at))
  response.release(state('c1', 0))
  assert.equal(await request, true)
  assert.equal(count('c1'), 0)
})

test('three reads of one key form a queue, not a crowd waiting on one promise', async () => {
  // Several mark-reads pile up on one key while the first is in flight: the view
  // re-runs on every message and every read state. If each waits on the SAME
  // in-flight promise they all wake together and go out at once, which is the
  // reordering this exists to prevent.
  const { reads } = owner()
  const gates = [held<ChannelReadState>(), held<ChannelReadState>(), held<ChannelReadState>()]
  const dispatched: number[] = []
  const done: number[] = []
  const running = gates.map((gate, i) =>
    reads.serialize('c1', async (at) => {
      dispatched.push(i)
      const body = await gate.promise
      done.push(i)
      return reads.applyIf(body, reads.epoch, at)
    }))

  await Promise.resolve()
  assert.deepEqual(dispatched, [0], 'more than one read of this key was in flight')
  for (let i = 0; i < gates.length; i++) {
    gates[i]!.release(state('c1', 0))
    await running[i]
    await Promise.resolve()
    assert.deepEqual(done, [...Array(i + 1).keys()], 'a read finished out of order')
    assert.deepEqual(dispatched, [...Array(Math.min(i + 2, gates.length)).keys()], 'dispatch did not follow the queue')
  }
})

test('one key never blocks or invalidates another', async () => {
  const { reads, count } = owner()
  const slow = held<ChannelReadState>()
  const a = reads.serialize('c1', async (at) => reads.applyIf(await slow.promise, reads.epoch, at))
  assert.equal(await reads.serialize('c2', async (at) => reads.applyIf(state('c2', 0), reads.epoch, at)), true)
  assert.equal(count('c2'), 0)
  slow.release(state('c1', 2))
  assert.equal(await a, true, "another channel's update invalidated this one")
  assert.equal(count('c1'), 2)
})

// --- Resync ordering, both directions --------------------------------------

test('a resync snapshot that returns FIRST waits for the acknowledgement and loses', async () => {
  // The observed failure: snapshot at version 0, a read dispatched at 0, the old
  // snapshot lands first and bumps the version, and the newer acknowledgement is
  // then rejected as stale. Displayed count was the snapshot's 6.
  const { reads, count } = owner()
  const epoch = reads.epoch
  const versionsAt = reads.snapshot()
  const response = held<ChannelReadState>()
  let readApplied: boolean | undefined
  const reading = reads.serialize('c1', async (at) => { readApplied = reads.applyIf(await response.promise, epoch, at) })

  const snapshot = reads.reconcile([state('c1', 6)], epoch, versionsAt)
  await Promise.resolve()
  assert.equal(count('c1'), undefined, 'the snapshot applied without waiting for the read')

  response.release(state('c1', 0))
  await reading
  await snapshot
  assert.equal(readApplied, true, 'the newer acknowledgement was rejected by an older snapshot')
  assert.equal(count('c1'), 0, 'the resync snapshot won over a newer acknowledgement')
})

test('a resync snapshot that returns LAST also loses to the acknowledgement', async () => {
  const { reads, count } = owner()
  const epoch = reads.epoch
  const versionsAt = reads.snapshot()
  assert.equal(await reads.serialize('c1', async (at) => reads.applyIf(state('c1', 0), epoch, at)), true)
  await reads.reconcile([state('c1', 6), state('c2', 4)], epoch, versionsAt)
  assert.equal(count('c1'), 0, 'the resync snapshot overwrote a newer read')
  // A key the snapshot is not stale for still lands, so this is not an assertion
  // that resync never applies anything.
  assert.equal(count('c2'), 4)
})

test('a resync with nothing in flight applies normally', async () => {
  const { reads, count } = owner()
  await reads.reconcile([state('c1', 3)], reads.epoch, reads.snapshot())
  assert.equal(count('c1'), 3)
})

test('a resync drops a channel that is gone, but not one updated since the snapshot', async () => {
  const { reads, map, count } = owner()
  reads.apply(state('deleted', 4))
  reads.apply(state('kept', 1))
  const epoch = reads.epoch
  const versionsAt = reads.snapshot()
  // A socket update for `kept` arrives while the resync GETs are in flight, and
  // the snapshot does not list it. It is newer, so it stays.
  reads.apply(state('kept', 9))

  await reads.reconcile([state('other', 0)], epoch, versionsAt)
  assert.equal(map().has('deleted'), false, 'a deleted channel kept its count forever')
  assert.equal(count('kept'), 9, 'a channel updated after the snapshot was dropped anyway')
  assert.equal(count('other'), 0)
})

test('forgetting a key advances its version instead of resetting it', async () => {
  // A pending request holds a number. If removal reset the key to zero, that
  // request would come back still holding a matching token and win.
  const { reads, map } = owner()
  reads.apply(state('gone', 4))
  const stale = reads.version('gone')
  await reads.reconcile([], reads.epoch, reads.snapshot())
  assert.equal(map().has('gone'), false)
  assert.notEqual(reads.version('gone'), stale, 'the version was reused after removal')
  assert.equal(reads.applyIf(state('gone', 4), reads.epoch, stale), false, 'a stale request repopulated a removed key')
  assert.equal(map().has('gone'), false)
})

// --- Account lifetime -------------------------------------------------------

test('a reset clears the counts themselves, not only their versions', () => {
  const { reads, map } = owner()
  reads.apply(state('c1', 5))
  reads.apply(state('c2', 2))
  reads.reset()
  assert.deepEqual([...map().keys()], [], 'the previous account left its counts behind')
  assert.equal(reads.version('c1'), 0)
})

test('a reset drops work that was queued but never dispatched', async () => {
  const { reads, map } = owner()
  const first = held<ChannelReadState>()
  const dispatched: string[] = []
  const a = reads.serialize('c1', async () => { dispatched.push('a'); await first.promise })
  const queued = reads.serialize('c1', async (at) => {
    dispatched.push('queued')
    return reads.applyIf(state('c1', 9), reads.epoch, at)
  })
  reads.reset()
  first.release(state('c1', 0))
  await a
  assert.equal(await queued, undefined, 'work queued for the old account was dispatched anyway')
  assert.deepEqual(dispatched, ['a'])
  assert.deepEqual([...map().keys()], [], 'a cleared account was refilled')
})

test('a reset rejects a response that was already in flight', async () => {
  const { reads, map } = owner()
  const response = held<ChannelReadState>()
  const epoch = reads.epoch
  const request = reads.serialize('c1', async (at) => reads.applyIf(await response.promise, epoch, at))
  reads.reset()
  response.release(state('c1', 5))
  assert.equal(await request, false)
  assert.deepEqual([...map().keys()], [])
})

test('a reset rejects a resync snapshot that was already in flight', async () => {
  const { reads, map } = owner()
  const epoch = reads.epoch
  const versionsAt = reads.snapshot()
  reads.reset()
  await reads.reconcile([state('c1', 7)], epoch, versionsAt)
  assert.deepEqual([...map().keys()], [], "the previous account's resync refilled this one")
})

test('a failed request releases its key for the next read', async () => {
  const { reads, count } = owner()
  const broken = held<ChannelReadState>()
  const failing = reads.serialize('c1', async () => {
    try { await broken.promise } catch { /* the store records readError here */ }
  })
  broken.fail(new Error('offline'))
  await failing
  assert.equal(await reads.serialize('c1', async (at) => reads.applyIf(state('c1', 1), reads.epoch, at)), true)
  assert.equal(count('c1'), 1)
})

test('a held acknowledgement on one key does not stall the rest of a resync', async () => {
  // The Store calls reconcile from the `resync` event handler, which runs inside
  // the single WebSocket event chain. If one channel's queue held the whole
  // snapshot, a slow acknowledgement there would stop events for every channel.
  const { reads, count } = owner()
  const stuck = held<ChannelReadState>()
  const epoch = reads.epoch
  const versionsAt = reads.snapshot()
  const blocked = reads.serialize('c1', async (at) => reads.applyIf(await stuck.promise, epoch, at))

  // Not awaited as a whole: it cannot finish until c1's queue drains, which is
  // exactly why the Store does not await it either.
  const snapshot = reads.reconcile([state('c1', 6), state('c2', 4), state('c3', 2)], epoch, versionsAt)
  await Promise.resolve()
  assert.equal(count('c2'), 4, 'a busy channel blocked an unrelated one')
  assert.equal(count('c3'), 2)
  assert.equal(count('c1'), undefined, 'the snapshot jumped the queue it was supposed to wait in')

  stuck.release(state('c1', 0))
  assert.equal(await blocked, true)
  await snapshot
  assert.equal(count('c1'), 0, 'the acknowledgement lost to the older snapshot behind it')
})
