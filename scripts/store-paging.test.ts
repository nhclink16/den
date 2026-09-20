import { test } from 'node:test'
import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { createRequire } from 'node:module'
import { fileURLToPath } from 'node:url'
import { runInNewContext } from 'node:vm'

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
const globals = {
  ...compile(read('read-state.ts')),
  ...compile(read('message-fetch.ts')),
  $state: cell, apiFor: () => api, PAGE, native: false,
  Uploads: class { clear() {} }, Drafts: class { token = 0; clear() { this.token++ } },
  cachedAppearance: () => ({}), loadLayout: () => ({}),
  noSpotify: () => ({ connection: 'unavailable', account_name: null, connected_at: null, expires_at: null }),
  call: { snapshot() {}, applyAV() {}, leave: async () => {} }, objects: {},
  themes: { receive() {} }, localStorage: { setItem() {}, getItem: () => null },
  appendNew: (a: string[], b: string[]) => [...new Set([...a, ...b])], byActivity: (a: unknown) => a,
  activeListeners: new Set(), setCsrf() {},
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
