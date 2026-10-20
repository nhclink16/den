import { test } from 'node:test'
import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { accentBubble, bubbleFill, linkInk, nameInk, personColor, places, tint, TIME_BREAK, withContrast } from '../apps/web/src/lib/bubbles.ts'
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

// What a 9% ink wash (code, mentions) over a colour comes to, as the browser draws it.
const wash = (bg: string, ink: string) => '#' + [1, 3, 5].map((i) => {
  const a = parseInt(bg.slice(i, i + 2), 16), b = parseInt(ink.slice(i, i + 2), 16)
  return Math.round(a + (b - a) * .09).toString(16).padStart(2, '0')
}).join('')

test('text on every bubble colour reads at 4.5:1 in every theme, both halves, odd custom palettes and any Contrast', () => {
  for (const [name, raw] of palettes) for (const percent of [80, 100, 120]) {
    const c = withContrast(raw as never, percent) as Record<string, string>
    // A palette (or a Contrast setting) whose own raised surface reads below 4.5
    // is the person's choice; a coloured bubble must not make it worse.
    // Text sits on the bubble and on the ink wash behind code; secondary text only on the bubble.
    const on = (fill: string) => [[c.ink!, fill], [c.ink!, wash(fill, c.ink!)], [c.ink2!, fill]] as const
    const floor = on(c.bg3!).map(([ink, bg]) => Math.min(4.5, contrast(ink, bg)))
    for (const color of colors) {
      const fill = tint(color, c as never)
      on(fill).forEach(([ink, bg], i) => assert.ok(contrast(ink, bg) >= floor[i]! - .02, `${name} @${percent} ${color} → ${bg}: ${contrast(ink, bg).toFixed(2)}`))
    }
  }
})

test('Contrast moves secondary text the way the page does', () => {
  const c = { bg: '#ffffff', ink: '#000000', ink2: '#808080' }
  assert.equal(withContrast(c, 100), c)
  // 20 points below 100 mixes 11% of the way toward the background.
  assert.equal(withContrast(c, 80).ink2, '#8e8e8e')
  assert.equal(withContrast(c, 120).ink2, '#727272')
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
  // Everyone else shares the grey (their colour is on their name); yours is solid accent.
  for (const mode of ['everyone', 'plain'] as const) {
    assert.equal(fill(mode, false), undefined)
    assert.equal(fill(mode, true), undefined)
    assert.ok(accentBubble(mode))
  }
  // "Only mine" keeps your own colour on your bubbles.
  assert.equal(fill('mine', false), undefined)
  assert.equal(fill('mine', true), tint(person, c))
  assert.ok(!accentBubble('mine'))
  // A message that names you is lit in the accent in every mode, so it still stands out.
  for (const mode of ['everyone', 'mine', 'plain'] as const) assert.equal(fill(mode, false, true), tint(accent, c))
  // An older appearance without the field reads as everyone's colours.
  assert.equal(bubbleFill(undefined, false, false, person, accent, c), undefined)
  assert.ok(accentBubble(undefined))
})

test('text on your own accent bubble reads at 4.5:1 in every built-in theme, both halves, with every accent', () => {
  // The bubble is the accent and its text the page colour (--on-lamp), as for every accent fill.
  for (const t of themes) for (const h of halves) for (const choice of [null, ...accentPresets.map((p) => p.id)]) {
    const c = t[h], accent = resolveAccent(c, choice, h)
    assert.ok(contrast(c.bg, accent) >= 4.5, `${t.id} ${h} ${choice ?? 'theme'}: ${contrast(c.bg, accent).toFixed(2)}`)
    // Links and mentions on everyone else's grey bubble.
    assert.ok(contrast(linkInk(accent, c), c.bg3) >= 4.5, `link ${t.id} ${h} ${choice ?? 'theme'}`)
  }
})

test("a chosen profile colour is used as is; without one it follows the picture's hue", () => {
  assert.equal(personColor('x', '#AA3355'), '#aa3355')
  assert.equal(personColor('01M4RRCJ06DX0SPKJKD78390E9', null), personColor('01M4RRCJ06DX0SPKJKD78390E9'))
  assert.match(personColor('01M4RRCJ06DX0SPKJKD78390E9', 'junk'), /^#[0-9a-f]{6}$/)
})
