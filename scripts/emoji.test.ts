import { test } from 'node:test'
import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { fullyQualified, pushRecent, searchEmoji, toEntries, type RawEmoji } from '../apps/web/src/lib/emoji.ts'

const raw: RawEmoji[] = JSON.parse(readFileSync(new URL('../apps/web/node_modules/emojibase-data/en/data.json', import.meta.url), 'utf8'))
const entries = toEntries(raw)
const top = (q: string, n = 1) => searchEmoji(entries, q).slice(0, n).map((e) => e.emoji)

test('picked emoji match what the quick reactions send', () => {
  // A different byte form of the same emoji would count as a second reaction.
  const all = new Set(entries.map((e) => e.emoji))
  for (const quick of ['👍', '😂', '❤️', '🔥', '👀', '💀']) assert.ok(all.has(quick), `${quick} is in the picker as-is`)
  assert.equal(fullyQualified({ hexcode: '1F44D', type: 1 }), '👍', 'no stray FE0F on emoji-style code points')
  assert.equal(fullyQualified({ hexcode: '2764', type: 0 }), '❤️', 'text-style code points get FE0F')
  assert.equal(fullyQualified({ hexcode: '1F3F3-FE0F-200D-26A7-FE0F', type: 1 }), '🏳️‍⚧️', 'sequences keep their own FE0Fs')
})

test('the picker lists real emoji only, in categories, and drops what the device cannot draw', () => {
  assert.ok(!entries.some((e) => e.name.startsWith('regional indicator')), 'no bare flag letters')
  assert.ok(!entries.some((e) => /skin tone$/.test(e.name) && [...e.emoji].length === 1), 'no bare skin tone swatches')
  assert.equal(entries[0]!.category, 'Smileys & emotion')
  assert.equal(entries.at(-1)!.category, 'Flags')
  const version = new Map(raw.map((r) => [fullyQualified(r), r.version]))
  const old = toEntries(raw, 13)
  assert.ok(old.length < entries.length && old.every((e) => version.get(e.emoji)! <= 13))
})

test('search finds by name, keyword and partial word, best match first', () => {
  assert.deepEqual(top('fire'), ['🔥'])
  assert.deepEqual(top(':fire:'), ['🔥'], 'shortcode colons are ignored')
  assert.deepEqual(top('Thumbs Up'), ['👍'])
  assert.deepEqual(top('red heart'), ['❤️'])
  assert.ok(top('lol', 5).includes('😂'), 'keywords count')
  assert.ok(top('pizz', 3).includes('🍕'), 'a word still being typed matches')
  assert.ok(top('skull', 3).includes('💀'))
  assert.deepEqual(top('zzqqx'), [])
  assert.deepEqual(searchEmoji(entries, '   '), [])
  assert.ok(searchEmoji(entries, 'face').length <= 160, 'results are capped')
})

test('recent emoji move to the front once, and the list stays short', () => {
  assert.deepEqual(pushRecent([], '🔥'), ['🔥'])
  assert.deepEqual(pushRecent(['👍', '🔥', '😂'], '🔥'), ['🔥', '👍', '😂'])
  const full = Array.from({ length: 24 }, (_, i) => String(i))
  const next = pushRecent(full, 'new')
  assert.equal(next.length, 24)
  assert.equal(next[0], 'new')
  assert.ok(!next.includes('23'), 'the oldest falls off')
})
