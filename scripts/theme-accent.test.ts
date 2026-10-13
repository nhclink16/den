import { test } from 'node:test'
import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { accentPresets, contrast, deriveHalf, resolveAccent } from '../apps/web/src/lib/theme-color.ts'

const themes = JSON.parse(readFileSync(new URL('../crates/den-core/src/themes.json', import.meta.url), 'utf8'))
const halves = ['light', 'dark'] as const
const surfaces = (c: Record<string, string>) => [c.bg!, c.bg2!, c.bg3!]

test('every accent choice reads at 4.5:1 on and beside it, in every theme and half', () => {
  // Custom picks include the hard ones: near the background in either half.
  const choices = [...accentPresets.map((p) => p.id), '#ffffff', '#000000', '#ffff00', '#777777', '#1d63d8', '#E8A44A']
  for (const theme of themes) for (const half of halves) for (const choice of choices) {
    const colors = theme[half]
    const accent = resolveAccent(colors, choice, half)
    assert.match(accent, /^#[0-9a-f]{6}$/, `${theme.id} ${half} ${choice}`)
    // Links and counts sit next to it on every surface; button labels use bg on it.
    for (const bg of surfaces(colors)) assert.ok(contrast(accent, bg) >= 4.5, `${theme.id} ${half} ${choice} on ${bg}: ${contrast(accent, bg).toFixed(2)}`)
  }
})

test('a preset that already reads stays exactly as designed', () => {
  const neutral = themes.find((t: { id: string }) => t.id === 'neutral')
  for (const preset of accentPresets) for (const half of halves) assert.equal(resolveAccent(neutral[half], preset.id, half), preset[half])
})

test("no choice, an old appearance without the field, or junk keeps the theme's own accent", () => {
  for (const theme of themes) for (const half of halves) for (const choice of [null, undefined, '', 'purple', '#12ab3']) {
    assert.equal(resolveAccent(theme[half], choice, half), theme[half].accent)
  }
})

test('generated halves still pass after sharing the readability search', () => {
  for (const theme of themes) for (const half of halves) {
    const derived = deriveHalf(theme[half])
    for (const role of ['accent', 'success', 'danger'] as const) assert.ok(contrast(derived[role], derived.bg) >= 4.5, `${theme.id} ${half} ${role}`)
  }
})
