import { test } from 'node:test'
import assert from 'node:assert/strict'
import { emojiOnly } from '../apps/web/src/lib/markdown.ts'
import { clockTime } from '../apps/web/src/lib/time.ts'

test('only short all-emoji messages are drawn large', () => {
  for (const big of ['🔥', '😂😂', '👍🏽', '👨‍👩‍👧‍👦 ❤️ 🇨🇦']) assert.ok(emojiOnly(big), big)
  // Digits and # are Emoji_Component, so a naive class would enlarge "1" or "#1".
  for (const small of ['', ' ', '1', '#1', 'ok 👍', '🔥🔥🔥🔥', ':)']) assert.ok(!emojiOnly(small), JSON.stringify(small))
})

test('the gutter clock drops the day period but keeps the locale separator', () => {
  const at = '2026-09-22T20:03:00'
  assert.equal(clockTime(at, 'en-US'), '8:03')
  assert.equal(clockTime(at, 'de'), '20:03')
  assert.equal(clockTime(at, 'fi'), '20.03')
})
