import { test } from 'node:test'
import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { bubbleFill, nameInk, personColor, places, tint, TIME_BREAK } from '../apps/web/src/lib/bubbles.ts'
import { accentPresets, contrast, deriveHalf, resolveAccent } from '../apps/web/src/lib/theme-color.ts'

const themes = JSON.parse(readFileSync(new URL('../crates/den-core/src/themes.json', import.meta.url), 'utf8'))
const halves = ['light', 'dark'] as const

const at = (minutes: number) => new Date(Date.UTC(2026, 9, 13, 12, 0) + minutes * 60_000).toISOString()
const msg = (id: string, author: string, minutes: number) => ({ id, author_id: author, created_at: at(minutes) })
const shape = (list: ReturnType<typeof msg>[], unread?: string) =>
  places(list, unread).map((p) => `${p.time ? 'T' : ''}${p.head ? 'H' : ''}${p.tail ? 'E' : ''}` || '-')

test("one person's messages in a row stack under one name and one picture", () => {
  const list = [msg('1', 'a', 0), msg('2', 'a', 1), msg('3', 'a', 2), msg('4', 'b', 3), msg('5', 'a', 4)]
  assert.deepEqual(shape(list), ['TH', '-', 'E', 'HE', 'HE'])
})

test('a long pause or a new day shows the time and starts a new stack', () => {
  const quiet = TIME_BREAK / 60_000
  const list = [msg('1', 'a', 0), msg('2', 'a', quiet - 1), msg('3', 'a', 2 * quiet), msg('4', 'a', 2 * quiet + 1)]
  assert.deepEqual(shape(list), ['TH', 'E', 'TH', 'E'])
  // Midnight splits a burst even when it was only a minute.
  const late = [{ id: '1', author_id: 'a', created_at: '2026-10-13T23:59:30' }, { id: '2', author_id: 'a', created_at: '2026-10-14T00:00:10' }]
  assert.deepEqual(shape(late), ['THE', 'THE'])
})

test('the unread line starts a new stack without showing the time', () => {
  const list = [msg('1', 'a', 0), msg('2', 'a', 1), msg('3', 'a', 2)]
  assert.deepEqual(shape(list, '2'), ['THE', 'H', 'E'])
})

test('an empty list and a single message', () => {
  assert.deepEqual(places([]), [])
  assert.deepEqual(shape([msg('1', 'a', 0)]), ['THE'])
})

// The awkward ones: near either background, pure primaries, greys.
const colors = ['#ffffff', '#000000', '#ffff00', '#00ffff', '#ff0000', '#777777', '#1d63d8', '#e8a44a', personColor('01M4RRCJ06DX0SPKJKD78390E9')]
// Custom palettes people could save: low-contrast ink, and an ink2 that only just passes.
const custom = [
  { bg: '#101010', bg2: '#181818', bg3: '#202020', line: '#333333', ink: '#9a9a9a', ink2: '#8a8a8a', ink3: '#888888', accent: '#7aa2ff', success: '#33cc33', danger: '#ff5555' },
  { bg: '#fafafa', bg2: '#f0f0f0', bg3: '#e6e6e6', line: '#cccccc', ink: '#5c5c5c', ink2: '#666666', ink3: '#666666', accent: '#1d63d8', success: '#137333', danger: '#c81e1e' },
]
const palettes = [
  ...themes.flatMap((t: Record<string, Record<string, string>>) => halves.map((h) => [`${t.id} ${h}`, t[h]])),
  ...themes.map((t: Record<string, Record<string, string>>) => [`${t.id} derived`, deriveHalf(t.dark as never)]),
  ...custom.map((c, i) => [`custom ${i}`, c]),
] as [string, Record<string, string>][]

test('text on every bubble colour reads at 4.5:1 in every theme, both halves, and odd custom palettes', () => {
  for (const [name, c] of palettes) {
    // A palette whose own raised surface fails is the author's choice; the bubble must not make it worse.
    const floor = (ink: string) => Math.min(4.5, contrast(ink, c.bg3!))
    for (const color of colors) {
      const fill = tint(color, c as never)
      for (const ink of [c.ink!, c.ink2!]) assert.ok(contrast(ink, fill) >= floor(ink) - 1e-9, `${name} ${color} → ${fill}: ${contrast(ink, fill).toFixed(2)}`)
    }
  }
})

test('with no bubbles, coloured names read at 4.5:1 on the page and sidebar colours', () => {
  for (const [name, c] of palettes) for (const color of colors) {
    const ink = nameInk('everyone', false, color, c as never)!
    for (const bg of [c.bg!, c.bg2!]) assert.ok(contrast(ink, bg) >= 4.5, `${name} ${color} → ${ink} on ${bg}`)
  }
  const c = palettes[0]![1] as never
  assert.equal(nameInk('plain', true, '#d17465', c), undefined)
  assert.equal(nameInk('mine', false, '#d17465', c), undefined)
  assert.ok(nameInk('mine', true, '#d17465', c))
})

test('the guard leaves the colour showing in every built-in theme', () => {
  for (const t of themes) for (const h of halves) {
    const c = t[h]
    for (const color of ['#d17465', '#17a1c8', '#3fcf7a']) assert.notEqual(tint(color, c), c.bg3, `${t.id} ${h} ${color}`)
  }
})

test('each colour mode paints the right bubbles', () => {
  const c = themes.find((t: { id: string }) => t.id === 'neutral').light
  const accent = resolveAccent(c, accentPresets[0]!.id, 'light')
  const person = '#d17465'
  const fill = (mode: 'everyone' | 'mine' | 'plain', mine: boolean, mentions = false) => bubbleFill(mode, mine, mentions, person, accent, c)
  assert.equal(fill('everyone', false), tint(person, c))
  assert.equal(fill('everyone', true), tint(person, c))
  assert.equal(fill('mine', false), undefined)
  assert.equal(fill('mine', true), tint(person, c))
  assert.equal(fill('plain', false), undefined)
  assert.equal(fill('plain', true), tint(accent, c))
  // A message that names you is lit in the accent in every mode, so it still stands out.
  for (const mode of ['everyone', 'mine', 'plain'] as const) assert.equal(fill(mode, false, true), tint(accent, c))
  // An older appearance without the field reads as everyone's colours.
  assert.equal(bubbleFill(undefined, false, false, person, accent, c), tint(person, c))
})

test("a chosen profile colour is used as is; without one it follows the picture's hue", () => {
  assert.equal(personColor('x', '#AA3355'), '#aa3355')
  assert.equal(personColor('01M4RRCJ06DX0SPKJKD78390E9', null), personColor('01M4RRCJ06DX0SPKJKD78390E9'))
  assert.match(personColor('01M4RRCJ06DX0SPKJKD78390E9', 'junk'), /^#[0-9a-f]{6}$/)
})
