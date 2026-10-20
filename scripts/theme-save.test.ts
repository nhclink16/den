import { test } from 'node:test'
import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { createRequire } from 'node:module'
import { fileURLToPath } from 'node:url'
import { runInNewContext } from 'node:vm'

// Runs the real Themes class (theme.svelte.ts) with its runes and modules stubbed,
// and reads what each save sends to the server: that copy is what other devices get.

const require_ = createRequire(import.meta.url)
const ts = require_('../apps/web/node_modules/typescript')
const lib = fileURLToPath(new URL('../apps/web/src/lib/', import.meta.url))
const builtins = JSON.parse(readFileSync(new URL('../crates/den-core/src/themes.json', import.meta.url), 'utf8'))

function compile(file: string, modules: Record<string, unknown>, globals: Record<string, unknown> = {}) {
  const exports: Record<string, any> = {}
  const js = ts.transpileModule(readFileSync(lib + file, 'utf8'), {
    compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022 },
  }).outputText
  runInNewContext(js, { ...globals, exports, require: (m: string) => modules[m], JSON, Promise, Error, TextEncoder, structuredClone, Object, Array, Set, console })
  return exports
}

/** A server that keeps one account's appearance. A save sets the fields it names. `hold` makes
 *  the next save wait, before or after the server takes it, until `release()`. */
function server(initial: Record<string, unknown>) {
  const sent: any[] = []
  let state: any = structuredClone(initial), held: (() => void) | null = null, hold: 'before' | 'after' | null = null
  const wait = () => new Promise<void>(resolve => { held = resolve })
  return {
    sent, get state() { return state },
    hold(when: 'before' | 'after') { hold = when },
    release() { const go = held; held = null; go?.() },
    /** Another device saves: returns the copy the socket then brings. */
    other(patch: Record<string, unknown>) { state = { ...state, ...patch }; return structuredClone(state) },
    api: {
      put: async (_path: string, body: any) => {
        sent.push(structuredClone(body))
        const when = hold; hold = null
        if (when === 'before') await wait()
        state = { ...state, ...structuredClone(body) }
        const reply = structuredClone(state)
        if (when === 'after') await wait()
        return reply
      },
      get: async () => structuredClone(state),
    },
  }
}
const tick = () => new Promise(resolve => setTimeout(resolve, 0))

function setup(start: Record<string, unknown>, systemLight = false, servers: Record<string, ReturnType<typeof server>> = { 'https://den.test': server(start) }, origin = { now: 'https://den.test' }) {
  const color = compile('theme-color.ts', {})
  const runtime = {
    builtinThemes: builtins,
    defaultAppearance: { mode: 'system', light_theme: 'neutral', dark_theme: 'neutral', custom_themes: [], contrast: 100, accent: null },
    themeFor: (a: any, half: string) => [...builtins, ...a.custom_themes].find((t: any) => t.id === (half === 'light' ? a.light_theme : a.dark_theme)) ?? builtins[0],
    appearanceHalf: (a: any, light: boolean) => (a.mode === 'light' || (a.mode === 'system' && light) ? 'light' : 'dark'),
    applyTheme() {}, appearanceCacheKey: () => 'den.appearance', fontStack: () => '', systemFont: 'System',
    cachedAppearance: () => structuredClone(start),
  }
  const { themes } = compile('theme.svelte.ts', {
    './api': { apiFor: (o: string) => servers[o].api }, './native': { activeOrigin: () => origin.now },
    './theme-runtime': runtime, './theme-color': color, './avatar-carry': { forgetOldAvatarChoice() {} },
  }, {
    $state: (v: unknown) => v,
    matchMedia: () => ({ matches: systemLight, addEventListener() {} }),
    localStorage: { setItem() {} }, document: { querySelector: () => null },
  })
  // What the server has after each save: that copy is what other devices get.
  return { themes, saved: () => servers[origin.now].state, sent: servers[origin.now].sent }
}
// Made from Neutral, whose fonts are the device's own: nothing to fetch from Google Fonts.
const mine = (id: string, radius = 'soft') => ({ ...structuredClone(builtins.find((t: any) => t.id === 'neutral')), id, name: id, radius })
const base = { mode: 'system', contrast: 100, accent: null, background: null, sidebar_background: null }

test('a theme saved while one theme served both goes on both, so a device on the other half shows it too', async () => {
  const { themes, saved } = setup({ ...base, light_theme: 'neutral', dark_theme: 'neutral', custom_themes: [] })
  await themes.add(mine('plum'))
  assert.equal(saved().light_theme, 'plum')
  assert.equal(saved().dark_theme, 'plum')
})

test('a split choice keeps its other half: the new theme goes on the half being shown', async () => {
  const { themes, saved } = setup({ ...base, light_theme: 'neutral', dark_theme: 'sky', custom_themes: [mine('sky')] })
  await themes.add(mine('plum'))
  assert.deepEqual([saved().light_theme, saved().dark_theme], ['neutral', 'plum'])
  // The phone's editor showing the light half saves onto light.
  themes.showHalf('light')
  await themes.add(mine('rose'))
  assert.deepEqual([saved().light_theme, saved().dark_theme], ['rose', 'plum'])
  themes.showHalf(null)
  assert.equal(themes.half, 'dark')
})

test('changing your own theme saves it in place, with the choices and other themes untouched', async () => {
  const { themes, sent } = setup({ ...base, light_theme: 'neutral', dark_theme: 'plum', custom_themes: [mine('plum'), mine('rose')] })
  await themes.update({ ...mine('plum'), radius: 'sharp', density: 'compact' })
  const a = sent.at(-1)
  assert.deepEqual(a.custom_themes.map((t: any) => [t.id, t.radius, t.density]), [['plum', 'sharp', 'compact'], ['rose', 'soft', 'comfortable']])
  assert.deepEqual(Object.keys(a), ['custom_themes'])
  assert.deepEqual([themes.appearance.light_theme, themes.appearance.dark_theme], ['neutral', 'plum'])
  assert.equal(themes.draft, null)
})

// Overlapping saves. Every Appearance autosave goes through one queue; these are the
// cases where it used to write an old copy over another device's change.
const start = { ...base, light_theme: 'neutral', dark_theme: 'neutral', custom_themes: [] }
const one = (s: ReturnType<typeof server>, origin = { now: 'https://den.test' }) => setup(start, false, { [origin.now]: s }, origin).themes

test('two devices changing different settings at nearly the same time both keep their change', async () => {
  const s = server(start), themes = one(s)
  s.hold('before')
  const saved = themes.accent('green')
  await tick()
  // The phone saves light mode while this save is still on its way to the server.
  const frame = s.other({ mode: 'light' })
  s.release(); await saved
  assert.equal(s.state.mode, 'light')
  assert.equal(s.state.accent, 'green')
  themes.receive(frame); themes.receive(structuredClone(s.state))
  assert.deepEqual([themes.appearance.mode, themes.appearance.accent], ['light', 'green'])
})

test("another device's change shows while a save is on its way, and later saves keep it", async () => {
  const s = server(start), themes = one(s)
  s.hold('before')
  const first = themes.accent('green')
  await tick()
  themes.receive(s.other({ mode: 'light', room_icons: 'hash' }))
  assert.deepEqual([themes.appearance.mode, themes.appearance.room_icons, themes.appearance.accent], ['light', 'hash', 'green'])
  const second = themes.contrast(110)
  s.release(); await first; await second
  assert.deepEqual([s.state.mode, s.state.room_icons, s.state.accent, s.state.contrast], ['light', 'hash', 'green', 110])
  assert.deepEqual([themes.appearance.mode, themes.appearance.accent, themes.appearance.contrast], ['light', 'green', 110])
})

test("a save's reply doesn't replace a newer copy that came over the socket", async () => {
  const s = server(start), themes = one(s)
  s.hold('after')
  const saved = themes.accent('green')
  await tick()
  // The server took the save, then the phone's change, and its copy arrives before the reply.
  themes.receive(s.other({ mode: 'light' }))
  s.release(); await saved
  assert.deepEqual([themes.appearance.mode, themes.appearance.accent], ['light', 'green'])
})

test('a device that missed a change while offline does not undo it with its next save', async () => {
  const s = server(start), themes = one(s)
  // The phone saves while this device's socket is down: no copy arrives here.
  s.other({ mode: 'light', bubble_shape: 'none' })
  await themes.roomIcons('hash')
  assert.deepEqual([s.state.mode, s.state.bubble_shape, s.state.room_icons], ['light', 'none', 'hash'])
})

test('a save queued for one account is dropped when another signs in, and never shows there', async () => {
  const s = server(start), themes = one(s)
  let me = 'ann'
  themes.owner = () => `https://den.test|${me}`
  s.hold('before')
  const first = themes.accent('green')
  const second = themes.roomIcons('hash')
  await tick()
  me = 'bea'
  const bea = { ...start, mode: 'dark' }
  themes.receive(bea)
  assert.deepEqual([themes.appearance.accent, themes.appearance.room_icons, themes.appearance.mode], [null, undefined, 'dark'])
  s.release()
  assert.equal(await first, true)
  assert.equal(await second, false)
  assert.equal(s.sent.length, 1)
  assert.deepEqual([themes.appearance.accent, themes.appearance.room_icons, themes.appearance.mode], [null, undefined, 'dark'])
})

test('desktop with two servers: a switch drops the queued save, and the new server shows its own copy', async () => {
  const a = server(start), b = server({ ...start, mode: 'dark' }), origin = { now: 'https://a.test' }
  const themes = setup(start, false, { 'https://a.test': a, 'https://b.test': b }, origin).themes
  themes.owner = () => `${origin.now}|ann`
  a.hold('after')
  const first = themes.accent('green')
  const second = themes.roomIcons('hash')
  await tick()
  origin.now = 'https://b.test'
  themes.receive(structuredClone(b.state))
  // B's own phone changes something: it shows at once, while A's save is still out.
  themes.receive(b.other({ bubble_shape: 'none' }))
  assert.deepEqual([themes.appearance.mode, themes.appearance.bubble_shape, themes.appearance.accent], ['dark', 'none', null])
  a.release(); await first; await second
  assert.deepEqual([a.sent.length, b.sent.length], [1, 0])
  assert.equal(a.state.accent, 'green')
  assert.deepEqual([themes.appearance.mode, themes.appearance.bubble_shape, themes.appearance.accent, themes.appearance.room_icons], ['dark', 'none', null, undefined])
})
