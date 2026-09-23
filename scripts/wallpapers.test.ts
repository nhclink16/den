import { test } from 'node:test'
import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { wallpapers, wallpaperImage, legacyWallpapers } from '../apps/web/src/lib/wallpapers.ts'

const themes = JSON.parse(readFileSync(new URL('../crates/den-core/src/themes.json', import.meta.url), 'utf8'))

test('every preset paints in every built-in half', () => {
  for (const theme of themes) for (const half of ['light', 'dark']) for (const name of wallpapers) {
    const image = wallpaperImage(name, theme[half])
    assert.notEqual(image, 'none', `${theme.id} ${half} ${name}`)
    assert.ok(!/undefined|NaN/.test(image), `${theme.id} ${half} ${name} leaked a bad value`)
  }
})

test('SVG presets keep working internal references', () => {
  // Encoding `url(%23id)` a second time breaks every gradient and filter silently.
  for (const name of wallpapers) {
    const image = wallpaperImage(name, themes[0].dark)
    if (!image.startsWith('url("data:image/svg+xml,')) continue
    const svg = decodeURIComponent(image.slice('url("data:image/svg+xml,'.length, -2))
    for (const [, id] of svg.matchAll(/url\(#([^)]+)\)/g)) assert.match(svg, new RegExp(`id='${id}'`), `${name} #${id}`)
    assert.ok(!svg.includes('%23'), `${name} double-encoded a reference`)
  }
})

test('names saved before the redesign still paint', () => {
  for (const [old, now] of Object.entries(legacyWallpapers)) assert.equal(wallpaperImage(old, themes[0].dark), wallpaperImage(now, themes[0].dark), old)
  assert.equal(wallpaperImage('nonsense', themes[0].dark), 'none')
})
