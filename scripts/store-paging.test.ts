import { test } from 'node:test'
import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { createRequire } from 'node:module'
import { fileURLToPath } from 'node:url'
import { runInNewContext } from 'node:vm'
import { endJam } from '../apps/web/src/lib/jam.ts'

// These tests reach the ACTUAL Store, not a helper that mirrors it.
//
// Every other test around the message-fetch rule builds MessageFetches and
// applyRange directly, and that is exactly why none of them could see the bug
// below: the registry was correct and the Store's USE of it was not. No test
// written at the registry's level could have failed. So this one pays a small
// harness cost to run the real store.svelte.ts, with its rune cells and its
// external services stubbed.
//
// What that makes it: a wiring test over real Store methods and real page
// application. What it is not: a browser, network, or rendering claim.

const require_ = createRequire(import.meta.url)
const ts = require_('../apps/web/node_modules/typescript')
const lib = fileURLToPath(new URL('../apps/web/src/lib/', import.meta.url))
const read = (f: string) => readFileSync(lib + f, 'utf8')

function compile(src: string, globals: Record<string, unknown> = {}) {
  const exports: Record<string, any> = {}
  const js = ts.transpileModule(src, {
    compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022 },
  }).outputText
  runInNewContext(js, { ...globals, exports, Map, Set, Promise, URLSearchParams, console, setTimeout, clearTimeout })
  return exports
}

// PAGE is deliberately 2 so a two-message page is a FULL page. A short page
// would set `exhausted` and block the next older load for an unrelated reason,
// which would let these tests pass or fail for the wrong cause.
const PAGE = 2
const source = read('store.svelte.ts')
const cell = (x: unknown) => x
;(cell as any).raw = cell
let api: any
const sockets: any[] = []
const globals = {
  ...compile(read('read-state.ts')),
  ...compile(read('message-fetch.ts')),
  $state: cell, apiFor: () => api, PAGE, native: false,
  Uploads: class { clear() {} }, Drafts: class { token = 0; clear() { this.token++ } },
  cachedAppearance: () => ({}), loadLayout: () => ({}), carryOldAvatarChoices: async () => {},
  noSpotify: () => ({ connection: 'unavailable', account_name: null, connected_at: null, expires_at: null }),
  call: { snapshot() {}, applyAV() {}, leave: async () => {} }, objects: {},
  themes: { receive() {} }, localStorage: { setItem() {}, getItem: () => null },
  appendNew: (a: string[], b: string[]) => [...new Set([...a, ...b])], byActivity: (a: unknown) => a,
  activeListeners: new Set(), setCsrf() {}, pushSignedOut() {},
  WebSocket: class { onmessage: any; onclose: any; onopen: any; constructor() { sockets.push(this) } send() {} close() { this.onclose?.() } },
  lastOf: (m: any) => ({ id: m.id, author_id: m.author_id, created_at: '', content: m.content, attachment_types: [], object_kinds: [] }),
}
const classStart = source.indexOf('export class Store {')
const classEnd = source.indexOf('\nclass Instances {')
if (classStart < 0 || classEnd < 0) throw new Error('Store class boundary moved; this harness needs updating')
const { Store } = compile(source.slice(classStart, classEnd), globals)

const held = () => {
  let release!: (v: unknown) => void
  const promise = new Promise((r) => { release = r })
  return { promise, release: (v: unknown) => release(v) }
}
const room = (id: string) => ({ id, channel_id: 'c', thread_id: null, author_id: 'u', content: id, reactions: [], edited_at: null })
const reply = (id: string) => ({ ...room(id), thread_id: 't' })

/// A room whose tail is loaded and which is mid-way through paging history.
function pagingRoom() {
  const pages: ReturnType<typeof held>[] = []
  const paths: string[] = []
  api = {
    get: async (p: string) => {
      paths.push(p)
      if (!p.includes('before=')) return [room('100'), room('110')]
      const h = held(); pages.push(h); return h.promise
    },
    post: async () => null,
  }
  const s = new Store('fixture')
  s.messages.set('c', [room('040'), room('050')])
  return { store: s, pages, paths, olderRequests: () => paths.filter((p) => p.includes('before=')).length }
}

test('a latest load clears the paging flag it just orphaned', async () => {
  // The sequence is ordinary: the user scrolls up in a busy room, and a resync
  // refreshes that room while the history page is still in flight.
  const r = pagingRoom()
  const pending = r.store.loadOlder('c')
  assert.ok(r.store.loadingOlder.has('c'), 'the older page never registered as loading')

  await r.store.loadLatest('c')          // supersedes the older request
  r.pages[0]!.release([room('020'), room('030')])
  await pending

  // The superseded request's `finally` correctly declines to clear this flag,
  // because by then the flag may belong to a newer request. If the replacement
  // does not clear it, nobody does: MessageList renders a permanent spinner and
  // refuses to load any more history for the rest of the session.
  assert.equal(r.store.loadingOlder.has('c'), false, 'the superseded older page left its spinner set forever')

  // And the consequence that the flag exists to gate: history still pages.
  assert.equal(r.olderRequests(), 1)
  void r.store.loadOlder('c')
  assert.equal(r.olderRequests(), 2, 'scrolling up again issued no request; the room can never load history again')
})

test('an older page that is never superseded clears its own flag', async () => {
  // The control. It passes with or without the fix, which is the point: only
  // the superseded ordering above discriminates.
  const r = pagingRoom()
  const pending = r.store.loadOlder('c')
  r.pages[0]!.release([room('020'), room('030')])
  await pending
  assert.equal(r.store.loadingOlder.has('c'), false)
  // Joined because the array is built inside the vm realm, so a strict deep
  // compare would fail on prototype identity rather than on content.
  assert.equal(r.store.messages.get('c').map((m: any) => m.id).join(), '020,030,040,050')
})

test('a superseded response still cannot clear a newer request\'s flag', async () => {
  // The guard the fix must not undo. Clearing on replacement is safe only
  // because a late response continues to check that the flag is its own.
  const r = pagingRoom()
  const first = r.store.loadOlder('c')
  await r.store.loadLatest('c')
  const second = r.store.loadOlder('c')        // a NEW older request owns the flag now
  assert.ok(r.store.loadingOlder.has('c'))

  r.pages[0]!.release([room('020'), room('030')])   // the cancelled one lands late
  await first
  assert.ok(r.store.loadingOlder.has('c'), 'a cancelled response cleared a newer request\'s spinner')

  r.pages[1]!.release([room('020'), room('030')])
  await second
  assert.equal(r.store.loadingOlder.has('c'), false)
})

test('a thread latest load clears the paging flag it just orphaned', async () => {
  // Same defect, same shape, the other conversation kind — reached through the
  // thread's own Store methods rather than assumed to follow from the room's.
  const pages: ReturnType<typeof held>[] = []
  api = {
    get: async (p: string) => {
      if (!p.includes('before=')) return [reply('100'), reply('110')]
      const h = held(); pages.push(h); return h.promise
    },
    post: async () => null,
  }
  const s = new Store('fixture')
  s.threadMessages.set('t', [reply('040'), reply('050')])
  const pending = s.loadOlderThreadReplies('t')
  assert.ok(s.loadingOlder.has('t'))

  await s.loadThreadMessages('t')
  pages[0]!.release([reply('020'), reply('030')])
  await pending
  assert.equal(s.loadingOlder.has('t'), false, 'the superseded older replies left their spinner set forever')
})

test('logout leaves no paging flag for the next account', async () => {
  const r = pagingRoom()
  const pending = r.store.loadOlder('c')
  assert.ok(r.store.loadingOlder.has('c'))
  await r.store.logout()
  assert.equal(r.store.loadingOlder.size, 0, 'a paging flag survived logout and would wedge the next account')
  r.pages[0]!.release([room('020'), room('030')])
  await pending
})

test('a Jam read from the previous account cannot land in the next account', async () => {
  const old = held()
  const fresh = held()
  let reads = 0
  api = {
    get: async () => (++reads === 1 ? old.promise : fresh.promise),
    post: async () => null,
  }
  const s = new Store('fixture')
  const oldRead = s.loadJam('c')

  // These are the synchronous account-boundary changes made by logout. The
  // next account can see the same room id and starts its own first read.
  s.generation++
  s.jamSeq.clear()
  s.jams = new Map()
  const freshRead = s.loadJam('c')

  old.release({ jam: { id: 'old-account-jam' } })
  await oldRead
  assert.equal(s.jams.size, 0, 'the previous account populated the new account cache')

  fresh.release({ jam: { id: 'new-account-jam' } })
  await freshRead
  assert.equal(s.jams.get('c')?.id, 'new-account-jam')
})

test('a Spotify account read from the previous account cannot land in the next account', async () => {
  const old = held()
  api = {
    get: async () => old.promise,
    post: async () => null,
  }
  const s = new Store('fixture')
  const oldRead = s.loadSpotify()

  s.generation++
  s.spotify = { connection: 'unavailable' }
  old.release({ connection: 'connected', account_name: 'old-account' })
  await oldRead

  assert.equal(s.spotify.connection, 'unavailable', 'the previous account repopulated Spotify state')
})

test('a Spotify socket update beats a read that started before it', async () => {
  const stale = held()
  api = {
    get: async () => stale.promise,
    post: async () => null,
  }
  const s = new Store('fixture')
  const read = s.loadSpotify()

  await s.handle({ type: 'spotify_account_updated', account: { connection: 'disconnected' } })
  stale.release({ connection: 'connected', account_name: 'stale-account' })
  await read

  assert.equal(s.spotify.connection, 'disconnected', 'a stale read overwrote the socket update')
})

test('a Jam mutation response cannot overwrite a newer socket event', async () => {
  const stale = held()
  api = { get: async () => null, post: async () => null }
  const s = new Store('fixture')
  const write = s.mutateJam('c', () => stale.promise)

  s.receiveJam('c', { id: 'newer-socket-jam' })
  stale.release({ id: 'stale-http-jam' })
  assert.equal(await write, true)
  assert.equal(s.jams.get('c')?.id, 'newer-socket-jam')
})

test('Jam and Spotify mutation responses cannot cross an account boundary', async () => {
  const jam = held()
  const spotify = held()
  api = { get: async () => null, post: async () => null }
  const s = new Store('fixture')
  const jamWrite = s.mutateJam('c', () => jam.promise)
  const spotifyWrite = s.mutateSpotify(() => spotify.promise)

  s.generation++
  s.jams = new Map()
  s.jamSeq.clear()
  s.receiveSpotify({ connection: 'unavailable' })
  jam.release({ id: 'previous-account-jam' })
  spotify.release({ connection: 'connected', account_name: 'previous-account' })

  assert.equal(await jamWrite, false)
  assert.equal(await spotifyWrite, false)
  assert.equal(s.jams.size, 0)
  assert.equal(s.spotify.connection, 'unavailable')
})

test('a Jam completion stays with the concrete instance that sent it', async () => {
  const ended = held()
  api = { get: async () => null, post: async () => null, del: async () => ended.promise }
  const a = new Store('instance-a')
  api = { get: async () => null, post: async () => null, del: async () => null }
  const b = new Store('instance-b')
  a.receiveJam('same-room', { id: 'jam-a' })
  b.receiveJam('same-room', { id: 'jam-b' })

  let active = a
  const proxy = new Proxy({} as any, {
    get(_target, key: keyof typeof a) {
      const value = active[key]
      return typeof value === 'function' ? value.bind(active) : value
    },
  })
  const pending = endJam(proxy, 'same-room')
  active = b
  ended.release(null)
  assert.equal(await pending, true)

  assert.equal(a.jams.has('same-room'), false)
  assert.equal(b.jams.get('same-room')?.id, 'jam-b')
})

// The room list's preview line: socket events build it, and a channel list
// fetched before one of them must not put an older message back.
const preview = (id: string) => ({ id, author_id: 'u', created_at: '', content: id, attachment_types: [], object_kinds: [] })
const lastIn = (s: any) => s.channels.find((c: any) => c.id === 'c').last_message?.id

test('a refetch after deleting the newest message cannot undo a message that arrived meanwhile', async () => {
  const fetches: ReturnType<typeof held>[] = []
  api = { get: async () => { const h = held(); fetches.push(h); return h.promise } }
  const s = new Store('fixture')
  s.channels = [{ id: 'c', kind: 'text', last_message: preview('B') }]
  await s.handle({ type: 'message_deleted', id: 'B', channel_id: 'c' })
  await s.handle({ type: 'message_created', ...room('C') })
  assert.equal(lastIn(s), 'C')
  fetches[0]!.release([{ id: 'c', kind: 'text', last_message: preview('A') }])   // answered before C existed
  await new Promise((r) => setTimeout(r, 0))
  assert.equal(lastIn(s), 'C', 'the stale refetch put the deleted message\'s predecessor back')
})

test('a resync that started before a new message keeps that message as the preview', async () => {
  const lists: ReturnType<typeof held>[] = []
  const empty: Record<string, unknown> = { '/presence': { online_user_ids: [], objects: [] }, '/users/me/notification-preferences': {}, '/settings': {}, '/users/me/appearance': {}, '/users/me/voice': {}, '/users/me/sounds': {}, '/users/me/spotify': {} }
  api = { get: async (p: string) => { if (p === '/channels') { const h = held(); lists.push(h); return h.promise } return p in empty ? empty[p] : [] }, put: async () => null }
  const s = new Store('fixture')
  s.channels = [{ id: 'c', kind: 'text', last_message: preview('B') }]
  const resync = s.resync()
  await s.handle({ type: 'message_created', ...room('C') })
  lists[0]!.release([{ id: 'c', kind: 'text', last_message: preview('B') }, { id: 'd', kind: 'text', last_message: null }])
  await resync
  assert.equal(lastIn(s), 'C', 'the resync snapshot overwrote a newer socket preview')
  assert.equal(s.channels.length, 2, 'the list itself still comes from the resync')
})

test('a reconnect that already brought the newest preview outranks a refetch still in flight', async () => {
  const lists: ReturnType<typeof held>[] = []
  const empty: Record<string, unknown> = { '/presence': { online_user_ids: [], objects: [] }, '/users/me/notification-preferences': {}, '/settings': {}, '/users/me/appearance': {}, '/users/me/voice': {}, '/users/me/sounds': {}, '/users/me/spotify': {} }
  api = { get: async (p: string) => { if (p === '/channels') { const h = held(); lists.push(h); return h.promise } return p in empty ? empty[p] : [] }, put: async () => null }
  const s = new Store('fixture')
  s.channels = [{ id: 'c', kind: 'text', last_message: preview('B') }]
  await s.handle({ type: 'message_deleted', id: 'B', channel_id: 'c' })   // refetch 0 waits
  const resync = s.resync()                                              // C arrived while offline
  lists[1]!.release([{ id: 'c', kind: 'text', last_message: preview('C') }])
  await resync
  lists[0]!.release([{ id: 'c', kind: 'text', last_message: preview('A') }])
  await new Promise((r) => setTimeout(r, 0))
  assert.notEqual(lastIn(s), 'A', 'the refetch from before the reconnect put an older message back')
  lists[2]?.release([{ id: 'c', kind: 'text', last_message: preview('C') }])
  await new Promise((r) => setTimeout(r, 0))
  assert.equal(lastIn(s), 'C')
})

test('replies in threads do not keep a deleted message on the list', async () => {
  const fetches: ReturnType<typeof held>[] = []
  api = { get: async () => { const h = held(); fetches.push(h); return h.promise } }
  const s = new Store('fixture')
  s.channels = [{ id: 'c', kind: 'text', last_message: preview('B') }]
  await s.handle({ type: 'message_deleted', id: 'B', channel_id: 'c' })
  for (const id of ['R1', 'R2', 'R3']) await s.handle({ type: 'message_created', ...reply(id) })
  fetches[0]!.release([{ id: 'c', kind: 'text', last_message: preview('A') }])
  await new Promise((r) => setTimeout(r, 0))
  assert.equal(lastIn(s), 'A', 'thread replies made the refetch give up and left the deleted message showing')
})

test('an older resync landing after a newer one keeps the newer previews', async () => {
  const lists: ReturnType<typeof held>[] = []
  const empty: Record<string, unknown> = { '/presence': { online_user_ids: [], objects: [] }, '/users/me/notification-preferences': {}, '/settings': {}, '/users/me/appearance': {}, '/users/me/voice': {}, '/users/me/sounds': {}, '/users/me/spotify': {} }
  api = { get: async (p: string) => { if (p === '/channels') { const h = held(); lists.push(h); return h.promise } return p in empty ? empty[p] : [] }, put: async () => null }
  const s = new Store('fixture')
  s.channels = [{ id: 'c', kind: 'text', last_message: preview('A') }]
  const first = s.resync(), second = s.resync()
  lists[0]!.release([{ id: 'c', kind: 'text', last_message: preview('B') }])
  await first
  lists[1]!.release([{ id: 'c', kind: 'text', last_message: preview('C') }])
  await second
  assert.equal(lastIn(s), 'C', 'the newer resync was ignored')
  const third = s.resync(), fourth = s.resync()
  lists[3]!.release([{ id: 'c', kind: 'text', last_message: preview('E') }])
  await fourth
  lists[2]!.release([{ id: 'c', kind: 'text', last_message: preview('D') }])
  await third
  assert.equal(lastIn(s), 'E', 'an older resync put its preview back')
})

test('a deletion that leaves the line alone cannot hold back a newer reconnect', async () => {
  const lists: ReturnType<typeof held>[] = []
  const empty: Record<string, unknown> = { '/presence': { online_user_ids: [], objects: [] }, '/users/me/notification-preferences': {}, '/settings': {}, '/users/me/appearance': {}, '/users/me/voice': {}, '/users/me/sounds': {}, '/users/me/spotify': {} }
  api = { get: async (p: string) => { if (p === '/channels') { const h = held(); lists.push(h); return h.promise } return p in empty ? empty[p] : [] }, put: async () => null }
  const s = new Store('fixture')
  s.channels = [{ id: 'c', kind: 'text', last_message: preview('B') }]
  const old = s.resync()
  await s.handle({ type: 'message_deleted', id: 'A', channel_id: 'c' })   // an older message goes
  const reconnect = s.resync()                                            // C arrived while offline
  lists[0]!.release([{ id: 'c', kind: 'text', last_message: preview('B') }])
  await old
  lists[1]!.release([{ id: 'c', kind: 'text', last_message: preview('C') }])
  await reconnect
  assert.equal(lastIn(s), 'C', 'the reconnect\'s newer line was thrown away')
})

test('an older resync landing after a newer one cannot bring back a cleared line', async () => {
  const lists: ReturnType<typeof held>[] = []
  const empty: Record<string, unknown> = { '/presence': { online_user_ids: [], objects: [] }, '/users/me/notification-preferences': {}, '/settings': {}, '/users/me/appearance': {}, '/users/me/voice': {}, '/users/me/sounds': {}, '/users/me/spotify': {} }
  api = { get: async (p: string) => { if (p === '/channels') { const h = held(); lists.push(h); return h.promise } return p in empty ? empty[p] : [] }, put: async () => null }
  const s = new Store('fixture')
  s.channels = [{ id: 'c', kind: 'text', last_message: preview('B') }]
  const old = s.resync(), reconnect = s.resync()
  lists[1]!.release([{ id: 'c', kind: 'text', last_message: null }])   // B was deleted while offline
  await reconnect
  lists[0]!.release([{ id: 'c', kind: 'text', last_message: preview('B') }])
  await old
  assert.equal(lastIn(s), undefined, 'the older list put the deleted message back')
})

test('events still queued on a replaced socket leave the new connection\'s previews alone', async () => {
  const lists: ReturnType<typeof held>[] = []
  const empty: Record<string, unknown> = { '/presence': { online_user_ids: [], objects: [] }, '/users/me/notification-preferences': {}, '/settings': {}, '/users/me/appearance': {}, '/users/me/voice': {}, '/users/me/sounds': {}, '/users/me/spotify': {} }
  api = { get: async (p: string) => { if (p === '/channels') { const h = held(); lists.push(h); return h.promise } return p in empty ? empty[p] : [] }, put: async () => null }
  const tick = () => new Promise((r) => setTimeout(r, 0))
  const s = new Store('fixture')
  s.me = { id: 'me' }; s.ready = true
  s.channels = [{ id: 'c', kind: 'text', last_message: { ...preview('B'), content: 'v0' } }]
  sockets.length = 0
  await s.connect()
  const old = sockets[0]
  const send = (ws: any, ev: unknown) => ws.onmessage({ data: JSON.stringify(ev) })
  send(old, { type: 'resync', reason: 'connect' })
  send(old, { type: 'message_edited', ...room('B'), content: 'v1' })      // queued behind the slow resync
  await tick()
  old.onclose()                                                          // drops; the edit to v2 is missed
  await s.connect()
  send(sockets[1], { type: 'resync', reason: 'connect' })
  await tick()
  lists[1]!.release([{ id: 'c', kind: 'text', last_message: { ...preview('B'), content: 'v2' } }])
  await tick(); await tick()
  sockets[1].onclose()                                                   // the newer socket drops too
  lists[0]!.release([{ id: 'c', kind: 'text', last_message: { ...preview('B'), content: 'v0' } }])
  for (let i = 0; i < 5; i++) await tick()
  assert.equal(s.channels.find((c: any) => c.id === 'c').last_message.content, 'v2', 'an old socket\'s queued edit overwrote the newer connection\'s line')
  s.ready = false   // stop the reconnect timer the closed socket scheduled
})

test('a socket frame that arrived before a newer list was asked for is already in that list', async () => {
  const lists: ReturnType<typeof held>[] = []
  const empty: Record<string, unknown> = { '/presence': { online_user_ids: [], objects: [] }, '/users/me/notification-preferences': {}, '/settings': {}, '/users/me/appearance': {}, '/users/me/voice': {}, '/users/me/sounds': {}, '/users/me/spotify': {} }
  api = { get: async (p: string) => { if (p === '/channels') { const h = held(); lists.push(h); return h.promise } return p in empty ? empty[p] : [] }, put: async () => null }
  const tick = () => new Promise((r) => setTimeout(r, 0))
  const s = new Store('fixture')
  s.me = { id: 'me' }; s.ready = true
  s.channels = [{ id: 'c', kind: 'text', last_message: { ...preview('B'), content: 'v0' } }]
  sockets.length = 0
  await s.connect()
  const send = (ev: unknown) => sockets[0].onmessage({ data: JSON.stringify(ev) })
  send({ type: 'resync', reason: 'connect' })
  send({ type: 'message_edited', ...room('B'), content: 'v1' })     // waits behind the slow resync
  await tick()
  const manual = s.resync()                                        // e.g. adding a category; sees v2
  lists[1]!.release([{ id: 'c', kind: 'text', last_message: { ...preview('B'), content: 'v2' } }])
  await manual
  lists[0]!.release([{ id: 'c', kind: 'text', last_message: { ...preview('B'), content: 'v0' } }])
  for (let i = 0; i < 5; i++) await tick()
  assert.equal(s.channels.find((c: any) => c.id === 'c').last_message.content, 'v2', 'a frame the newer list already covered was applied on top of it')
  s.ready = false
})

test('with no list in flight, message events are not kept around', async () => {
  api = { get: async () => [] }
  const s = new Store('fixture')
  s.channels = [{ id: 'c', kind: 'text', last_message: preview('B') }]
  for (let i = 0; i < 50; i++) await s.handle({ type: 'message_edited', ...room('B'), content: 'x'.repeat(1000) })
  assert.equal(s.previewLog.length, 0)
})

test('an edit that raced a list across a disconnect is put right by the reconnect', async () => {
  // Known limit: a frame that arrives after a list was asked for can still be
  // older than what that list read, if the socket then drops the newer frame.
  // The drop always means a reconnect, and the reconnect's list wins.
  const lists: ReturnType<typeof held>[] = []
  const empty: Record<string, unknown> = { '/presence': { online_user_ids: [], objects: [] }, '/users/me/notification-preferences': {}, '/settings': {}, '/users/me/appearance': {}, '/users/me/voice': {}, '/users/me/sounds': {}, '/users/me/spotify': {} }
  api = { get: async (p: string) => { if (p === '/channels') { const h = held(); lists.push(h); return h.promise } return p in empty ? empty[p] : [] }, put: async () => null }
  const tick = () => new Promise((r) => setTimeout(r, 0))
  const line = (s: any) => s.channels.find((c: any) => c.id === 'c').last_message.content
  const s = new Store('fixture')
  s.me = { id: 'me' }; s.ready = true
  s.channels = [{ id: 'c', kind: 'text', last_message: { ...preview('B'), content: 'v0' } }]
  sockets.length = 0
  await s.connect()
  sockets[0].onmessage({ data: JSON.stringify({ type: 'resync', reason: 'connect' }) })
  await tick()
  sockets[0].onmessage({ data: JSON.stringify({ type: 'message_edited', ...room('B'), content: 'v1' }) })
  sockets[0].onclose()                                                   // v2's frame never comes
  lists[0]!.release([{ id: 'c', kind: 'text', last_message: { ...preview('B'), content: 'v2' } }])
  for (let i = 0; i < 5; i++) await tick()
  assert.equal(line(s), 'v1', 'the known transient state; if this changes, the limit is gone')
  await s.connect()
  sockets[1].onmessage({ data: JSON.stringify({ type: 'resync', reason: 'connect' }) })
  await tick()
  lists[1]!.release([{ id: 'c', kind: 'text', last_message: { ...preview('B'), content: 'v2' } }])
  for (let i = 0; i < 5; i++) await tick()
  assert.equal(line(s), 'v2')
  s.ready = false
})

test('a room renamed or given an icon changes live and keeps its preview line', async () => {
  api = { get: async () => [] }
  const s = new Store('fixture')
  s.channels = [{ id: 'c', kind: 'text', name: 'general', icon: null, category_id: null, position: 0, member_ids: [], last_message: preview('B') }]
  await s.handle({ type: 'channel_updated', channel: { id: 'c', kind: 'text', name: 'lobby', icon: 'flame', category_id: 'k', position: 3, member_ids: [], last_message: null } })
  const c = s.channels[0]
  assert.deepEqual([c.name, c.icon, c.category_id, c.position], ['lobby', 'flame', 'k', 3])
  assert.equal(lastIn(s), 'B', 'the event carries no preview and must not clear the line')
  await s.handle({ type: 'channel_updated', channel: { id: 'gone', kind: 'text', name: 'x', icon: null, category_id: null, position: 0, member_ids: [], last_message: null } })
  assert.equal(s.channels.length, 1, 'a change to a room this list does not have adds nothing')
})

test('a resync that started before a room change keeps the change', async () => {
  const lists: ReturnType<typeof held>[] = []
  const empty: Record<string, unknown> = { '/presence': { online_user_ids: [], objects: [] }, '/users/me/notification-preferences': {}, '/settings': {}, '/users/me/appearance': {}, '/users/me/voice': {}, '/users/me/sounds': {}, '/users/me/spotify': {} }
  api = { get: async (p: string) => { if (p === '/channels') { const h = held(); lists.push(h); return h.promise } return p in empty ? empty[p] : [] }, put: async () => null }
  const s = new Store('fixture')
  const before = { id: 'c', kind: 'text', name: 'general', icon: null, category_id: null, position: 0, member_ids: [], last_message: preview('B') }
  s.channels = [before]
  const resync = s.resync()
  await s.handle({ type: 'channel_updated', channel: { ...before, name: 'lobby', icon: 'flame', last_message: null } })
  lists[0]!.release([before])                                       // read before the change
  await resync
  assert.deepEqual([s.channels[0].name, s.channels[0].icon], ['lobby', 'flame'], 'the older list put the old name back')
  assert.equal(s.roomLog.length, 0, 'nothing is kept once no list is in flight')
  // A list asked for after the change already has it, and a later rename is not undone by the log.
  const again = s.resync()
  lists[1]!.release([{ ...before, name: 'hangout-2', icon: 'flame' }])
  await again
  assert.equal(s.channels[0].name, 'hangout-2')
})

test('an older resync landing after a newer one keeps the newer room names', async () => {
  const lists: ReturnType<typeof held>[] = []
  const empty: Record<string, unknown> = { '/presence': { online_user_ids: [], objects: [] }, '/users/me/notification-preferences': {}, '/settings': {}, '/users/me/appearance': {}, '/users/me/voice': {}, '/users/me/sounds': {}, '/users/me/spotify': {} }
  api = { get: async (p: string) => { if (p === '/channels') { const h = held(); lists.push(h); return h.promise } return p in empty ? empty[p] : [] }, put: async () => null }
  const s = new Store('fixture')
  const room = { id: 'c', kind: 'text', name: 'general', icon: null, category_id: null, position: 0, member_ids: [], last_message: preview('B') }
  s.channels = [room]
  const older = s.resync()
  await s.handle({ type: 'channel_updated', channel: { ...room, name: 'lobby', last_message: null } })     // rename A
  const newer = s.resync()                                                                               // a reconnect: rename B's frame was lost
  lists[1]!.release([{ ...room, name: 'den' }])
  await newer
  lists[0]!.release([{ ...room, name: 'general' }, { ...room, id: 'd', name: 'new room' }])
  await older
  assert.equal(s.channels.find((c: any) => c.id === 'c').name, 'den', 'the older list brought back an older name')
  assert.equal(s.channels.length, 2, 'the older list still decides which rooms exist')
})

test('a room frame queued behind a slow resync cannot undo a newer list', async () => {
  const lists: ReturnType<typeof held>[] = []
  const empty: Record<string, unknown> = { '/presence': { online_user_ids: [], objects: [] }, '/users/me/notification-preferences': {}, '/settings': {}, '/users/me/appearance': {}, '/users/me/voice': {}, '/users/me/sounds': {}, '/users/me/spotify': {} }
  api = { get: async (p: string) => { if (p === '/channels') { const h = held(); lists.push(h); return h.promise } return p in empty ? empty[p] : [] }, put: async () => null }
  const tick = () => new Promise((r) => setTimeout(r, 0))
  const s = new Store('fixture')
  s.me = { id: 'me' }; s.ready = true
  const room = { id: 'c', kind: 'text', name: 'general', icon: null, category_id: null, position: 0, member_ids: [], last_message: preview('B') }
  s.channels = [room]
  sockets.length = 0
  await s.connect()
  const send = (ev: unknown) => sockets[0].onmessage({ data: JSON.stringify(ev) })
  send({ type: 'resync', reason: 'connect' })
  send({ type: 'channel_updated', channel: { ...room, name: 'lobby', icon: 'flame', last_message: null } })   // waits behind the resync
  await tick()
  const manual = s.resync()                                        // sees a later rename
  lists[1]!.release([{ ...room, name: 'den', icon: 'skull' }])
  await manual
  lists[0]!.release([{ ...room, name: 'lobby', icon: 'flame' }])
  for (let i = 0; i < 5; i++) await tick()
  const c = s.channels.find((x: any) => x.id === 'c')
  assert.deepEqual([c.name, c.icon], ['den', 'skull'], 'a frame the newer list already covered was applied on top of it')
  s.ready = false
})

test('an icon save reply cannot undo a later change from the socket', async () => {
  const reply = held()
  api = { get: async () => [], put: async () => reply.promise }
  const s = new Store('fixture')
  const room = { id: 'c', kind: 'text', name: 'general', icon: null, category_id: null, position: 0, member_ids: [], last_message: null }
  s.channels = [room]
  const saving = s.setChannelIcon('c', 'flame')
  await s.handle({ type: 'channel_updated', channel: { ...room, icon: 'flame' } })   // this save
  await s.handle({ type: 'channel_updated', channel: { ...room, icon: 'skull' } })   // another admin, after
  reply.release({ ...room, icon: 'flame' })
  await saving
  assert.equal(s.channels[0].icon, 'skull')
  // With no socket news, the reply is what there is.
  const alone = held(); api.put = async () => alone.promise
  const again = s.setChannelIcon('c', 'music'); alone.release({ ...room, icon: 'music' }); await again
  assert.equal(s.channels[0].icon, 'music')
})

test('a preview refetch that lands first also brings the room details an older resync keeps', async () => {
  const lists: ReturnType<typeof held>[] = []
  const empty: Record<string, unknown> = { '/presence': { online_user_ids: [], objects: [] }, '/users/me/notification-preferences': {}, '/settings': {}, '/users/me/appearance': {}, '/users/me/voice': {}, '/users/me/sounds': {}, '/users/me/spotify': {} }
  api = { get: async (p: string) => { if (p === '/channels') { const h = held(); lists.push(h); return h.promise } return p in empty ? empty[p] : [] }, put: async () => null }
  const tick = () => new Promise((r) => setTimeout(r, 0))
  const s = new Store('fixture')
  const room = { id: 'c', kind: 'text', name: 'general', icon: null, category_id: 'k', position: 0, member_ids: [], last_message: preview('B') }
  s.channels = [room]
  const resync = s.resync()                                                   // after deleting category k
  await s.handle({ type: 'message_deleted', id: 'B', channel_id: 'c' })        // its refetch is asked later
  lists[1]!.release([{ ...room, category_id: null, last_message: preview('A') }])
  await tick(); await tick()
  lists[0]!.release([{ ...room, category_id: null, last_message: preview('A') }])
  await resync
  assert.equal(s.channels[0].category_id, null, 'the room kept a deleted category and dropped out of the list')
  assert.equal(lastIn(s), 'A')
})

test('an icon save reply cannot undo a newer list that covered frames still queued', async () => {
  const lists: ReturnType<typeof held>[] = []
  const reply = held()
  const empty: Record<string, unknown> = { '/presence': { online_user_ids: [], objects: [] }, '/users/me/notification-preferences': {}, '/settings': {}, '/users/me/appearance': {}, '/users/me/voice': {}, '/users/me/sounds': {}, '/users/me/spotify': {} }
  api = { get: async (p: string) => { if (p === '/channels') { const h = held(); lists.push(h); return h.promise } return p in empty ? empty[p] : [] }, put: async () => reply.promise }
  const tick = () => new Promise((r) => setTimeout(r, 0))
  const s = new Store('fixture')
  s.me = { id: 'me' }; s.ready = true
  const room = { id: 'c', kind: 'text', name: 'general', icon: null, category_id: null, position: 0, member_ids: [], last_message: null }
  s.channels = [room]
  sockets.length = 0
  await s.connect()
  const send = (ev: unknown) => sockets[0].onmessage({ data: JSON.stringify(ev) })
  const saving = s.setChannelIcon('c', 'flame')
  send({ type: 'resync', reason: 'connect' })                                  // slow; everything queues behind it
  send({ type: 'channel_updated', channel: { ...room, icon: 'flame' } })
  send({ type: 'channel_updated', channel: { ...room, icon: 'skull' } })       // another admin
  await tick()
  const manual = s.resync()
  lists[1]!.release([{ ...room, icon: 'skull' }])
  await manual
  reply.release({ ...room, icon: 'flame' })
  await saving
  lists[0]!.release([{ ...room, icon: 'flame' }])
  for (let i = 0; i < 5; i++) await tick()
  assert.equal(s.channels[0].icon, 'skull')
  s.ready = false
})

test('an icon save during a dropped connection survives a list read before it landed', async () => {
  const lists: ReturnType<typeof held>[] = []
  const reply = held()
  const empty: Record<string, unknown> = { '/presence': { online_user_ids: [], objects: [] }, '/users/me/notification-preferences': {}, '/settings': {}, '/users/me/appearance': {}, '/users/me/voice': {}, '/users/me/sounds': {}, '/users/me/spotify': {} }
  api = { get: async (p: string) => { if (p === '/channels') { const h = held(); lists.push(h); return h.promise } return p in empty ? empty[p] : [] }, put: async () => reply.promise }
  const s = new Store('fixture')
  const room = { id: 'c', kind: 'text', name: 'general', icon: null, category_id: null, position: 0, member_ids: [], last_message: null }
  s.channels = [room]
  const saving = s.setChannelIcon('c', 'flame')   // the socket is down: its event never comes
  const resync = s.resync()                        // the reconnect's list, read before the save committed
  reply.release({ ...room, icon: 'flame' })
  await saving
  assert.equal(s.channels[0].icon, 'flame')
  lists[0]!.release([room])
  await resync
  assert.equal(s.channels[0].icon, 'flame', 'a list read before the save put the old icon back')
})

test('an older room frame still queued cannot undo an icon save, but keeps its rename', async () => {
  const lists: ReturnType<typeof held>[] = []
  const reply = held()
  const empty: Record<string, unknown> = { '/presence': { online_user_ids: [], objects: [] }, '/users/me/notification-preferences': {}, '/settings': {}, '/users/me/appearance': {}, '/users/me/voice': {}, '/users/me/sounds': {}, '/users/me/spotify': {} }
  api = { get: async (p: string) => { if (p === '/channels') { const h = held(); lists.push(h); return h.promise } return p in empty ? empty[p] : [] }, put: async () => reply.promise }
  const tick = () => new Promise((r) => setTimeout(r, 0))
  const s = new Store('fixture')
  s.me = { id: 'me' }; s.ready = true
  const room = { id: 'c', kind: 'text', name: 'general', icon: null, category_id: null, position: 0, member_ids: [], last_message: null }
  s.channels = [room]
  sockets.length = 0
  await s.connect()
  const send = (ev: unknown) => sockets[0].onmessage({ data: JSON.stringify(ev) })
  send({ type: 'resync', reason: 'connect' })                                          // slow
  await tick()
  send({ type: 'channel_updated', channel: { ...room, name: 'lobby', icon: null } })   // a rename, from before the save; queued
  const saving = s.setChannelIcon('c', 'flame')
  reply.release({ ...room, name: 'lobby', icon: 'flame' })
  await saving
  sockets[0].onclose()                                                                 // the save's own event is lost
  lists[0]!.release([{ ...room }])                                                     // read before both
  for (let i = 0; i < 5; i++) await tick()
  const c = s.channels[0]
  assert.deepEqual([c.name, c.icon], ['lobby', 'flame'])
  s.ready = false
})

test('an icon save reply never touches the name', async () => {
  const lists: ReturnType<typeof held>[] = []
  const reply = held()
  const empty: Record<string, unknown> = { '/presence': { online_user_ids: [], objects: [] }, '/users/me/notification-preferences': {}, '/settings': {}, '/users/me/appearance': {}, '/users/me/voice': {}, '/users/me/sounds': {}, '/users/me/spotify': {} }
  api = { get: async (p: string) => { if (p === '/channels') { const h = held(); lists.push(h); return h.promise } return p in empty ? empty[p] : [] }, put: async () => reply.promise }
  const s = new Store('fixture')
  const room = { id: 'c', kind: 'text', name: 'general', icon: null, category_id: null, position: 0, member_ids: [], last_message: null }
  s.channels = [room]
  const saving = s.setChannelIcon('c', 'flame')        // the socket is down
  const resync = s.resync()                            // the reconnect: the save, then another admin's rename
  lists[0]!.release([{ ...room, name: 'lobby', icon: 'flame' }])
  await resync
  reply.release({ ...room, icon: 'flame' })            // held up; still says general
  await saving
  assert.deepEqual([s.channels[0].name, s.channels[0].icon], ['lobby', 'flame'])
})

test('two icon saves in a row with the socket down end on the second', async () => {
  const replies = [held(), held()]
  let i = 0
  api = { get: async () => [], put: async () => replies[i++]!.promise }
  const s = new Store('fixture')
  const room = { id: 'c', kind: 'text', name: 'general', icon: null, category_id: null, position: 0, member_ids: [], last_message: null }
  s.channels = [room]
  const first = s.setChannelIcon('c', 'flame'), second = s.setChannelIcon('c', 'skull')
  replies[0]!.release({ ...room, icon: 'flame' }); await first
  replies[1]!.release({ ...room, icon: 'skull' }); await second
  assert.equal(s.channels[0].icon, 'skull', 'the first reply made the second look overtaken')
})

test('a DM opened by the previous account cannot land in the next account', async () => {
  const pending = held()
  api = { get: async () => [], post: (p: string) => (p === '/dms' ? pending.promise : Promise.resolve(null)) }
  const s = new Store('fixture')
  s.me = { id: 'a' }
  const opening = s.openDm(['b'])
  await s.logout()
  s.me = { id: 'z' }
  pending.release({ id: 'dm-a-b', kind: 'dm', member_ids: ['a', 'b'] })
  await assert.rejects(opening)
  assert.equal(s.channels.some((c: any) => c.id === 'dm-a-b'), false, "the previous account's DM joined the next account's rooms")
  assert.equal(s.messages.has('dm-a-b'), false)
})
