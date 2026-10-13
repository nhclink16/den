import { test } from 'node:test'
import assert from 'node:assert/strict'
import { emojiOnly, render } from '../apps/web/src/lib/markdown.ts'
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

test('code comes out apart from the prose around it, without blank lines for its fences', () => {
  const users = new Map()
  assert.equal(render('paste this:\n```sh\nls -la\n```\nworks', users),
    '<span class="prose">paste this:</span><pre><code>ls -la</code></pre><span class="prose">works</span>')
  // A blank line the person typed stays.
  assert.equal(render('a\n\n```\nx\n```', users), '<span class="prose">a<br>&nbsp;</span><pre><code>x</code></pre>')
  assert.equal(render('```\nonly code\n```', users), '<pre><code>only code</code></pre>')
})
