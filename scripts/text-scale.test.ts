import { test } from 'node:test'
import assert from 'node:assert/strict'
import { followTextSize, textScale } from '../apps/web/src/lib/text-scale.ts'

// The iPhone's Text Size, as Den Shell sends it (body size over the default 17pt). What must hold: the default and
// anything smaller stay at 1, XXL is followed, XXXL and the accessibility sizes stop at the cap, and junk from a
// shell reads as the default rather than breaking every font size on the page.

test('follows the phone up to the cap, never below the default', () => {
  assert.equal(textScale(17 / 17), 1, 'Large, the default')
  assert.equal(textScale(14 / 17), 1, 'xSmall stays at Den’s own size')
  assert.equal(textScale(19 / 17), 1.118, 'xLarge')
  assert.equal(textScale(21 / 17), 1.235, 'XXL')
  assert.equal(textScale(23 / 17), 1.3, 'XXXL is past the cap')
  assert.equal(textScale(53 / 17), 1.3, 'the largest accessibility size')
})

test('anything that is not a number reads as the default', () => {
  for (const junk of [undefined, null, '1.2', NaN, Infinity, -Infinity, {}, true]) assert.equal(textScale(junk), 1, String(junk))
})

function page(shell: object | undefined) {
  const set: string[] = []
  const g = globalThis as Record<string, unknown>
  g.window = { denShell: shell }
  g.document = { documentElement: { style: { setProperty: (name: string, value: string) => set.push(`${name}=${value}`) } } }
  return set
}

test('asks the shell once, then follows each change it sends', async () => {
  const shell: { textScale(): Promise<number>; onTextScale?: ((s: number) => void) | null } = { textScale: async () => 21 / 17, onTextScale: null }
  const set = page(shell)
  followTextSize()
  await new Promise((r) => setTimeout(r))
  assert.deepEqual(set, ['--text-scale=1.235'])
  shell.onTextScale!(28 / 17)
  shell.onTextScale!(1)
  assert.deepEqual(set, ['--text-scale=1.235', '--text-scale=1.3', '--text-scale=1'])
})

test('a browser, or a shell from before text size, changes nothing', async () => {
  const none = page(undefined)
  followTextSize()
  const older = page({ call: async () => null, onCall: null })
  followTextSize()
  await new Promise((r) => setTimeout(r))
  assert.deepEqual([none, older], [[], []])
})
