import { test } from 'node:test'
import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'

// Browsers compile the HTML `pattern` attribute with the `v` flag. Under `v`, an
// unescaped `-` inside a character class is a syntax error, and the browser discards
// the whole pattern rather than failing loudly — client-side validation just stops
// working. That shipped once; this keeps it from shipping again.
test('the signup username pattern compiles the way a browser compiles it', () => {
  const source = readFileSync('apps/web/src/ui/Login.svelte', 'utf8')
  const match = /const USERNAME_PATTERN = '([^']+)'/.exec(source)
  assert(match, 'USERNAME_PATTERN not found in Login.svelte')
  const pattern = match[1]!.replace(/\\\\/g, '\\')

  assert.doesNotThrow(() => new RegExp(`^${pattern}$`, 'v'), 'must compile with the v flag')

  // And it must still mean what the server means.
  const re = new RegExp(`^${pattern}$`, 'v')
  for (const good of ['Andy', 'andy', 'an.dy-2', 'bo_bby']) assert(re.test(good), `${good} should match`)
  for (const bad of ['.andy', 'andy.', '-andy', 'an dy', 'an@dy', 'ab']) assert(!re.test(bad), `${bad} should not match`)
})
