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

function setup(start: Record<string, unknown>, systemLight = false) {
  const sent: any[] = []
  const color = compile('theme-color.ts', {})
  const runtime = {
    builtinThemes: builtins,
    defaultAppearance: { mode: 'system', light_theme: 'neutral', dark_theme: 'neutral', custom_themes: [], contrast: 100, accent: null },
    themeFor: (a: any, half: string) => [...builtins, ...a.custom_themes].find((t: any) => t.id === (half === 'light' ? a.light_theme : a.dark_theme)) ?? builtins[0],
    appearanceHalf: (a: any, light: boolean) => (a.mode === 'light' || (a.mode === 'system' && light) ? 'light' : 'dark'),
    applyTheme() {}, appearanceCacheKey: () => 'den.appearance', fontStack: () => '', systemFont: 'System',
    cachedAppearance: () => structuredClone(start),
  }
  const api = { put: async (_path: string, body: unknown) => { sent.push(body); return body }, get: async () => sent.at(-1) }
  const { themes } = compile('theme.svelte.ts', {
    './api': { apiFor: () => api }, './native': { activeOrigin: () => 'https://den.test' },
    './theme-runtime': runtime, './theme-color': color,
  }, {
    $state: (v: unknown) => v,
    matchMedia: () => ({ matches: systemLight, addEventListener() {} }),
    localStorage: { setItem() {} }, document: { querySelector: () => null },
  })
  return { themes, sent }
}
// Made from Neutral, whose fonts are the device's own: nothing to fetch from Google Fonts.
const mine = (id: string, radius = 'soft') => ({ ...structuredClone(builtins.find((t: any) => t.id === 'neutral')), id, name: id, radius })
const base = { mode: 'system', contrast: 100, accent: null, background: null, sidebar_background: null }

test('a theme saved while one theme served both goes on both, so a device on the other half shows it too', async () => {
  const { themes, sent } = setup({ ...base, light_theme: 'neutral', dark_theme: 'neutral', custom_themes: [] })
  await themes.add(mine('plum'))
  assert.equal(sent.at(-1).light_theme, 'plum')
  assert.equal(sent.at(-1).dark_theme, 'plum')
})

test('a split choice keeps its other half: the new theme goes on the half being shown', async () => {
  const { themes, sent } = setup({ ...base, light_theme: 'neutral', dark_theme: 'sky', custom_themes: [mine('sky')] })
  await themes.add(mine('plum'))
  assert.deepEqual([sent.at(-1).light_theme, sent.at(-1).dark_theme], ['neutral', 'plum'])
  // The phone's editor showing the light half saves onto light.
  themes.showHalf('light')
  await themes.add(mine('rose'))
  assert.deepEqual([sent.at(-1).light_theme, sent.at(-1).dark_theme], ['rose', 'plum'])
  themes.showHalf(null)
  assert.equal(themes.half, 'dark')
})

test('changing your own theme saves it in place, with the choices and other themes untouched', async () => {
  const { themes, sent } = setup({ ...base, light_theme: 'neutral', dark_theme: 'plum', custom_themes: [mine('plum'), mine('rose')] })
  await themes.update({ ...mine('plum'), radius: 'sharp', density: 'compact' })
  const a = sent.at(-1)
  assert.deepEqual(a.custom_themes.map((t: any) => [t.id, t.radius, t.density]), [['plum', 'sharp', 'compact'], ['rose', 'soft', 'comfortable']])
  assert.deepEqual([a.light_theme, a.dark_theme], ['neutral', 'plum'])
  assert.equal(themes.draft, null)
})
