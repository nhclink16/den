import { test } from 'node:test'
import assert from 'node:assert/strict'
import { atBottom, landing } from '../apps/web/src/lib/landing.ts'

// A 900px tall view over 4000px of messages: the bottom is at 3100.
const H = 4000, V = 900, END = H - V

test('nothing unread opens at the bottom and counts as read', () => {
  assert.deepEqual(landing(undefined, H, V), { top: END, bottom: true })
})

test('unread that fits on screen opens at the bottom: nothing jumps', () => {
  // The "new" line is visible from the bottom, 300px below the view's top.
  assert.deepEqual(landing(END + 300, H, V), { top: END, bottom: true })
})

test('unread off screen opens at the "new" line and is not read yet', () => {
  const { top, bottom } = landing(1200, H, V)
  assert.ok(top < 1200 && top > 1050, `the line should sit near the top, got ${top}`)
  assert.equal(bottom, false, 'opening at the line must not acknowledge the tail')
})

test('a line just off the top opens at it and leaves the tail to reach', () => {
  // On screen from the bottom: open at the bottom, read.
  assert.deepEqual(landing(END + 20, H, V), { top: END, bottom: true })
  // 10px off the top: open at the line with context, so the newest message is
  // below the fold and stays unread until the reader gets there.
  assert.deepEqual(landing(END - 10, H, V), { top: END - 106, bottom: false })
})

test('a line at the very top of short history clamps to the start', () => {
  assert.deepEqual(landing(12, H, V), { top: 0, bottom: false })
})

test('a conversation shorter than the view is read wherever the line is', () => {
  assert.deepEqual(landing(100, 600, V), { top: 0, bottom: true })
})

test('only reaching the bottom counts as having seen the newest message', () => {
  assert.equal(atBottom(H, END, V), true)
  assert.equal(atBottom(H, END - 79, V), true)
  assert.equal(atBottom(H, END - 80, V), false)
  assert.equal(atBottom(H, 1200, V), false)
})
