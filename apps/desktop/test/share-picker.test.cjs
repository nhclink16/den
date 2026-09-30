const { test } = require('node:test')
const assert = require('node:assert/strict')
const { pickSession, captureAnswer, validatePickerFrame } = require('../electron/picker.cjs')

const screen = { id: 'screen:0:0', name: 'Entire screen' }
const editor = { id: 'window:1001:0', name: 'Editor' }
const game = { id: 'window:2002:0', name: 'Game' }

test('a window that appears on a thumbnail refresh can be shared', () => {
  const pick = pickSession()
  pick.offer([screen, editor])
  pick.offer([screen, editor, game])
  assert.equal(pick.choose(game.id), game)
  assert.equal(pick.choose(editor.id), editor)
})

test('an id the picker never offered is refused, and a cancel is not a refusal', () => {
  const pick = pickSession()
  pick.offer([screen])
  assert.equal(pick.choose('window:9999:0'), false)
  assert.equal(pick.choose(''), false)
  assert.equal(pick.choose('__proto__'), false)
  assert.equal(pick.choose(null), null)
  assert.equal(pick.choose(undefined), null)
})

test('cancel, timeout and refusal all answer the capture with {}', () => {
  const opts = { audio: true, audioRequested: true, platform: 'win32' }
  assert.deepEqual(captureAnswer(null, opts), {})
  assert.deepEqual(captureAnswer(false, opts), {})
})

test('loopback audio is asked for only on Windows, when requested and switched on', () => {
  assert.deepEqual(captureAnswer(screen, { audio: true, audioRequested: true, platform: 'win32' }), { video: screen, audio: 'loopback' })
  assert.deepEqual(captureAnswer(game, { audio: false, audioRequested: true, platform: 'win32' }), { video: game })
  assert.deepEqual(captureAnswer(game, { audio: true, audioRequested: false, platform: 'win32' }), { video: game })
  assert.deepEqual(captureAnswer(game, { audio: true, audioRequested: true, platform: 'linux' }), { video: game })
  assert.deepEqual(captureAnswer(game, { audio: true, audioRequested: true, platform: 'darwin' }), { video: game })
})

test('only the trusted main frame can open the picker', () => {
  const trusted = url => url === 'den://app/'
  const mainFrame = { url: 'den://app/' }
  assert.ok(validatePickerFrame(mainFrame, mainFrame, trusted))
  assert.ok(!validatePickerFrame(null, mainFrame, trusted))
  assert.ok(!validatePickerFrame({ url: 'den://app/' }, mainFrame, trusted), 'a subframe with the same url')
  const untrusted = { url: 'https://evil.test/' }
  assert.ok(!validatePickerFrame(untrusted, untrusted, trusted))
})
