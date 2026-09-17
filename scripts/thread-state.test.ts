import { test } from 'node:test'
import assert from 'node:assert/strict'
import { ReadState } from '../apps/web/src/lib/read-state.ts'
import { appendNew, byActivity } from '../apps/web/src/lib/thread-order.ts'
import type { ChannelReadState, ThreadReadState, ThreadSummary } from '../apps/web/src/lib/types.ts'

const room = (channel_id: string, unread_count: number, last_read_id = ''): ChannelReadState =>
  ({ channel_id, last_read_id, unread_count, mention_count: 0, notification_count: 0 })
const conv = (thread_id: string, channel_id: string, unread_count: number, following = true): ThreadReadState =>
  ({ thread_id, channel_id, last_read_id: null, unread_count, mention_count: 0, following })
const summary = (id: string, channel_id: string, last_activity_at: string, resolved = false): ThreadSummary => ({
  id, channel_id, root_message_id: `root-${id}`, title: `About ${id}`, created_by: 'u1',
  created_at: '2026-01-01T00:00:00Z', last_activity_at, reply_count: 1,
  resolved_at: resolved ? '2026-02-01T00:00:00Z' : null, resolved_by: resolved ? 'u1' : null,
})

function held<T>() {
  let release!: (v: T) => void
  const promise = new Promise<T>((r) => { release = r })
  return { promise, release }
}
/// The two owners the Store builds, over real maps.
function owners() {
  let rooms = new Map<string, ChannelReadState>()
  let convs = new Map<string, ThreadReadState>()
  return {
    rooms: new ReadState<ChannelReadState>({ get: () => rooms, set: (v) => { rooms = v } }, (s) => s.channel_id),
    convs: new ReadState<ThreadReadState>({ get: () => convs, set: (v) => { convs = v } }, (s) => s.thread_id),
    roomCount: (id: string) => rooms.get(id)?.unread_count,
    convCount: (id: string) => convs.get(id)?.unread_count,
    convMap: () => convs,
  }
}

test('a room and two conversations inside it never share a key', async () => {
  // ThreadReadState carries a channel_id as well as its thread_id. One owner
  // keyed on channel_id would have made these three the same row.
  const o = owners()
  o.rooms.apply(room('c1', 7))
  o.convs.apply(conv('t1', 'c1', 3))
  o.convs.apply(conv('t2', 'c1', 1))
  assert.equal(o.roomCount('c1'), 7, 'a conversation overwrote its own room')
  assert.equal(o.convCount('t1'), 3)
  assert.equal(o.convCount('t2'), 1)
  assert.equal(o.convMap().size, 2, 'two conversations in one room collapsed into one')

  // Reading one conversation leaves the other and the room alone.
  const at = o.convs.version('t1')
  o.convs.applyIf(conv('t1', 'c1', 0), o.convs.epoch, at)
  assert.equal(o.convCount('t1'), 0)
  assert.equal(o.convCount('t2'), 1, 'reading one conversation cleared another')
  assert.equal(o.roomCount('c1'), 7, 'reading a conversation cleared its room')
})

test('a socket update beats the stale acknowledgement it overtook, per conversation', async () => {
  const o = owners()
  const response = held<ThreadReadState>()
  const request = o.convs.serialize('t1', async (at) => o.convs.applyIf(await response.promise, o.convs.epoch, at))
  o.convs.apply(conv('t1', 'c1', 2))       // a new reply arrives while the read is open
  response.release(conv('t1', 'c1', 0))
  assert.equal(await request, false, 'the stale acknowledgement was applied over a newer count')
  assert.equal(o.convCount('t1'), 2)
})

test('two acknowledgements for one conversation cannot reorder with no socket', async () => {
  const o = owners()
  const first = held<ThreadReadState>(), second = held<ThreadReadState>()
  const dispatched: string[] = []
  const a = o.convs.serialize('t1', async (at) => { dispatched.push('a'); return o.convs.applyIf(await first.promise, o.convs.epoch, at) })
  const b = o.convs.serialize('t1', async (at) => { dispatched.push('b'); return o.convs.applyIf(await second.promise, o.convs.epoch, at) })
  await Promise.resolve()
  assert.deepEqual(dispatched, ['a'], 'both went out at once')
  first.release(conv('t1', 'c1', 3))
  assert.equal(await a, true)
  await Promise.resolve()
  second.release(conv('t1', 'c1', 0))
  assert.equal(await b, true)
  assert.equal(o.convCount('t1'), 0)
})

test('a page of conversations never deletes the ones it omits', async () => {
  // The lists are filtered and paginated. Resolving is not deleting, and a page
  // of open threads says nothing at all about the resolved ones.
  const o = owners()
  o.convs.apply(conv('open', 'c1', 1))
  o.convs.apply(conv('resolved-but-unread', 'c1', 4))
  const epoch = o.convs.epoch
  const versions = o.convs.snapshot()

  await o.convs.applyPage([conv('open', 'c1', 2)], epoch, versions)
  assert.equal(o.convCount('open'), 2)
  assert.equal(o.convCount('resolved-but-unread'), 4, 'a page of open threads forgot an unread resolved one')
  assert.equal(o.convMap().size, 2)
})

test('a stale page loses to a newer update, per key', async () => {
  const o = owners()
  const epoch = o.convs.epoch
  const versions = o.convs.snapshot()
  o.convs.apply(conv('t1', 'c1', 5))                 // socket, after the request went out
  await o.convs.applyPage([conv('t1', 'c1', 0), conv('t2', 'c1', 9)], epoch, versions)
  assert.equal(o.convCount('t1'), 5, 'a stale page overwrote a newer count')
  assert.equal(o.convCount('t2'), 9, 'the page was dropped wholesale instead of per key')
})

test('an account reset clears both owners and rejects work in flight', async () => {
  const o = owners()
  o.rooms.apply(room('c1', 3))
  o.convs.apply(conv('t1', 'c1', 3))
  const response = held<ThreadReadState>()
  const epoch = o.convs.epoch
  const request = o.convs.serialize('t1', async (at) => o.convs.applyIf(await response.promise, epoch, at))

  o.rooms.reset()
  o.convs.reset()
  response.release(conv('t1', 'c1', 0))

  assert.equal(await request, false)
  assert.equal(o.roomCount('c1'), undefined, 'the previous account left room counts behind')
  assert.equal(o.convMap().size, 0, 'the previous account left conversations behind')
})

test('a confirmed removal advances the key instead of reusing its number', async () => {
  const o = owners()
  o.convs.apply(conv('gone', 'c1', 2))
  const stale = o.convs.version('gone')
  o.convs.forget('gone')
  assert.equal(o.convMap().has('gone'), false)
  assert.equal(o.convs.applyIf(conv('gone', 'c1', 2), o.convs.epoch, stale), false,
    'a request from before the removal put it back')
})

// --- Strip order ------------------------------------------------------------

test('the strip holds its order for a visit and appends what it learns', () => {
  const meta = new Map([
    ['a', summary('a', 'c1', '2026-03-01T10:00:00Z')],
    ['b', summary('b', 'c1', '2026-03-01T12:00:00Z')],
    ['c', summary('c', 'c1', '2026-03-01T11:00:00Z')],
  ])
  const at = (id: string) => meta.get(id)
  // Entering the room: newest activity first.
  const entered = byActivity(['a', 'b', 'c'], at)
  assert.deepEqual(entered, ['b', 'c', 'a'])

  // A conversation becomes the busiest one while the reader is looking at the
  // strip. Its label may change; its position may not.
  meta.set('a', summary('a', 'c1', '2026-03-01T23:00:00Z'))
  assert.deepEqual(appendNew(entered, ['a']), ['b', 'c', 'a'], 'an entry moved under the pointer')

  // Something new appears: appended, not sorted in.
  meta.set('d', summary('d', 'c1', '2026-03-01T23:30:00Z'))
  assert.deepEqual(appendNew(entered, ['d']), ['b', 'c', 'a', 'd'])
  // The next room entry is where the order is recomputed.
  assert.deepEqual(byActivity(['b', 'c', 'a', 'd'], at), ['d', 'a', 'b', 'c'])
})

test('equal activity falls back to id, so the order is never arbitrary', () => {
  const same = '2026-03-01T10:00:00Z'
  const meta = new Map([['a', summary('a', 'c1', same)], ['b', summary('b', 'c1', same)]])
  assert.deepEqual(byActivity(['a', 'b'], (id) => meta.get(id)), ['b', 'a'])
})
