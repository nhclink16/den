import test from 'node:test'
import assert from 'node:assert/strict'
import { shareSource } from '../apps/web/src/lib/share-source.ts'

test('capture handles never become user-facing source names', () => {
  for (const raw of ['window:37438947', 'window:37438947:0', '37438947', '0x23b']) assert.equal(shareSource(raw, 'window').label, 'Window')
  for (const raw of ['screen:0:0', 'screen-2', '', '4fde445d-720e-4077-8040-2a13d9776e64']) assert.equal(shareSource(raw).label, 'Screen')
  assert.equal(shareSource('web-contents:12').label, 'Browser tab')
})
test('real source names survive with the capture surface glyph', () => {
  assert.deepEqual(shareSource('Helium — docs', 'browser'), { label: 'Helium — docs', surface: 'browser' })
  assert.deepEqual(shareSource('Terminal', 'window'), { label: 'Terminal', surface: 'window' })
  assert.deepEqual(shareSource('Screen 1', 'monitor'), { label: 'Screen 1', surface: 'monitor' })
  assert.equal(shareSource('', 'window').label, 'Window')
  assert.equal(shareSource('', 'browser').label, 'Browser tab')
})
