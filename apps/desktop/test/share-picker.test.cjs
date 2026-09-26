const { test } = require('node:test')
const assert = require('node:assert/strict')
const { validatePickerChoice, validatePickerFrame } = require('../electron/picker.cjs')

test('validatePickerChoice accepts offered source ids', () => {
  const ids = new Set(['screen:0:0', 'window:1:0'])
  assert.equal(validatePickerChoice(ids, 'screen:0:0'), 'screen:0:0')
  assert.equal(validatePickerChoice(ids, 'window:1:0'), 'window:1:0')
})

test('validatePickerChoice rejects ids not in the offered set', () => {
  const ids = new Set(['screen:0:0'])
  assert.equal(validatePickerChoice(ids, 'window:99:0'), false)
  assert.equal(validatePickerChoice(ids, ''), false)
  assert.equal(validatePickerChoice(ids, 'screen:0:0:extra'), false)
})

test('validatePickerChoice treats null/undefined as cancel and returns null', () => {
  const ids = new Set(['screen:0:0'])
  assert.equal(validatePickerChoice(ids, null), null)
  assert.equal(validatePickerChoice(ids, undefined), null)
})

test('validatePickerFrame rejects non-main-frame requests', () => {
  const trusted = url => url === 'den://app/'
  const mainFrame = { url: 'den://app/' }
  assert.ok(validatePickerFrame(mainFrame, mainFrame, trusted))
  assert.ok(!validatePickerFrame(null, mainFrame, trusted), 'null frame rejected')
  assert.ok(!validatePickerFrame(undefined, mainFrame, trusted), 'undefined frame rejected')
  const otherFrame = { url: 'den://app/' }
  assert.ok(!validatePickerFrame(otherFrame, mainFrame, trusted), 'different frame object rejected')
  assert.ok(!validatePickerFrame({ url: 'https://evil.com/' }, mainFrame, trusted), 'untrusted url rejected')
})

test('cancel (null sourceId) causes callback with {}', () => {
  const ids = new Set(['screen:0:0'])
  const result = validatePickerChoice(ids, null)
  assert.equal(result, null)
})
