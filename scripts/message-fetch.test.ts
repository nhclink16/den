import { test } from 'node:test'
import assert from 'node:assert/strict'
import {
  MessageFetches, applyRange, afterRange, beforeRange, cacheKey,
  type Op, type Request,
} from '../apps/web/src/lib/message-fetch.ts'
import type { Message } from '../apps/web/src/lib/types.ts'

const msg = (id: string, over: Partial<Message> = {}): Message => ({
  id, channel_id: 'c', author_id: 'u', content: id, created_at: '2026-01-01T00:00:00Z',
  attachments: [], ...over,
} as Message)
const reply = (id: string, threadId = 't') => msg(id, { thread_id: threadId })
const ids = (l: Message[]) => l.map((m) => m.id)

const roomReq = (mode: Request['mode'] = 'latest'): Request =>
  ({ account: 0, cache: { cache: 'room', channelId: 'c' }, mode })
const threadReq = (mode: Request['mode'] = 'latest'): Request =>
  ({ account: 0, cache: { cache: 'thread', threadId: 't' }, mode })

// --- What a page is allowed to say -----------------------------------------

test('a latest page replaces the whole list, so an empty one really clears it', () => {
  // This is how a reconnect repairs a deletion that happened while offline.
  assert.deepEqual(ids(applyRange([reply('010')], [], { all: true }, [], threadReq())), [])
})

test('an empty edge page is SHORT: it establishes the end it asked about', () => {
  // An empty before=050 proves there is nothing below 050 at all. Treating it as
  // an empty mathematical interval would leave stale entries sitting there.
  const before = applyRange([msg('010'), msg('050')], [], beforeRange([], '050', 25), [], roomReq('older'))
  assert.deepEqual(ids(before), ['050'])
  const after = applyRange([msg('050'), msg('090')], [], afterRange([], '050', 25), [], roomReq('older'))
  assert.deepEqual(ids(after), ['050'])
})

test('a FULL edge page proves only the span it returned', () => {
  // before=050 capped at 2 says nothing about anything older than its first id,
  // so 010 survives and the gap is not presented as loaded.
  const page = [msg('030'), msg('040')]
  const merged = applyRange([msg('010'), msg('050')], page, beforeRange(page, '050', 2), [], roomReq('older'))
  assert.deepEqual(ids(merged), ['010', '030', '040', '050'])
})

// --- What a response may admit ---------------------------------------------

test('an edge page never admits a far-newer arrival across an unloaded gap', () => {
  const ops: Op[] = [{ kind: 'put', message: msg('900') }]
  const page = [msg('030'), msg('040')]
  const merged = applyRange([msg('050')], page, beforeRange(page, '050', 2), ops, roomReq('older'))
  assert.ok(!ids(merged).includes('900'), 'an older page invented contiguous history')
})

test('a latest tail does admit one, and a room never takes a thread reply', () => {
  const arrival: Op[] = [{ kind: 'put', message: msg('020') }]
  assert.deepEqual(ids(applyRange([msg('010')], [msg('010')], { all: true }, arrival, roomReq())), ['010', '020'])
  const threadArrival: Op[] = [{ kind: 'put', message: reply('020') }]
  assert.deepEqual(ids(applyRange([msg('010')], [msg('010')], { all: true }, threadArrival, roomReq())), ['010'],
    'a threaded reply leaked into the room feed')
})

test('a live change beats the page, and an untouched message takes the fetched copy', () => {
  // The fetched copy is authoritative when nothing touched it: that is how a GET
  // repairs a reaction this client never saw arrive.
  const hydrated = msg('010', { reactions: [{ emoji: '👍', user_ids: ['u'] }] } as Partial<Message>)
  assert.deepEqual(
    applyRange([msg('010')], [hydrated], { all: true }, [], roomReq())[0]!.reactions?.length, 1)
  // But a deletion that crossed the request wins over the page that still has it.
  const deleted: Op[] = [{ kind: 'delete', id: '010' }]
  assert.deepEqual(ids(applyRange([], [hydrated], { all: true }, deleted, roomReq())), [],
    'a page resurrected a message deleted while it was in flight')
})

// --- Ordering and cancellation ---------------------------------------------

test('a replacement cancels the older request for the SAME cache', () => {
  // A latest load and an older page write the same tail, so they contend. The
  // older response must not come back and invent history the latest discarded.
  const f = new MessageFetches()
  const older = f.begin(threadReq('older'))
  const latest = f.begin(threadReq('latest'))
  assert.equal(f.valid(older), false, 'the superseded edge page is still wanted')
  assert.equal(f.valid(latest), true)
})

test('a room and a thread in one channel never contend', () => {
  const f = new MessageFetches()
  const room = f.begin(roomReq('latest'))
  f.begin(threadReq('latest'))
  assert.equal(f.valid(room), true, 'a thread load cancelled its room')
  assert.notEqual(cacheKey({ cache: 'room', channelId: 'c' }), cacheKey({ cache: 'thread', threadId: 'c' }))
})

test('finishing a replacement cannot make a superseded request valid again', () => {
  // The defect this pins: a lasting "newest" map gets pruned when the newer
  // request ends, and the older one silently becomes wanted again.
  const f = new MessageFetches()
  const first = f.begin(roomReq('latest'))
  const second = f.begin(roomReq('latest'))
  f.end(second)
  f.cancel((r) => r.cache.cache === 'room' && r.cache.channelId === 'elsewhere')
  assert.equal(f.valid(first), false, 'a superseded request came back to life')
})

test('cancellation is synchronous and end() reports it', () => {
  const f = new MessageFetches()
  const id = f.begin(threadReq('older'))
  f.record({ kind: 'put', message: reply('020') })
  f.cancel((r) => r.cache.cache === 'thread' && r.cache.threadId === 't')
  const { ok, ops } = f.end(id)
  assert.equal(ok, false, 'a cancelled request reported itself as wanted')
  assert.deepEqual(ops, [], 'a cancelled request still handed back operations to apply')
})

test('a replacement in flight is visible, so an edge page can refuse to dispatch', () => {
  const f = new MessageFetches()
  assert.equal(f.replacing({ cache: 'thread', threadId: 't' }), false)
  f.begin(threadReq('latest'))
  assert.equal(f.replacing({ cache: 'thread', threadId: 't' }), true)
  assert.equal(f.replacing({ cache: 'room', channelId: 'c' }), false, 'caches are not independent')
})

test('operations are recorded into every open fetch and released with it', () => {
  const f = new MessageFetches()
  const a = f.begin(roomReq('latest'))
  const b = f.begin(threadReq('latest'))
  f.record({ kind: 'put', message: msg('020') })
  assert.equal(f.end(a).ops.length, 1)
  assert.equal(f.end(b).ops.length, 1, 'a concurrent fetch missed a live change')
  f.record({ kind: 'put', message: msg('030') })
  assert.equal(f.end(a).ops.length, 0, 'operations outlived the fetch that needed them')
})
