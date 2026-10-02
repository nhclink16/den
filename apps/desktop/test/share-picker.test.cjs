const { test } = require('node:test')
const assert = require('node:assert/strict')
const { pickSession, picker, captureAnswer, validatePickerFrame, visibleSources } = require('../electron/picker.cjs')

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

// A fake 32x18 BGRA thumbnail filled with one colour, with optional lit pixels.
function image(bgra, lit = []) {
  const px = Buffer.alloc(32 * 18 * 4)
  for (let i = 0; i < px.length; i += 4) px.set(bgra, i)
  for (const [i, colour] of lit) px.set(colour, i * 4)
  return { isEmpty: () => false, toBitmap: () => px }
}
const windowWith = (name, thumbnail) => ({ id: `window:${name}:0`, name, thumbnail })

test('helper and overlay windows with blank thumbnails are hidden, dark real windows stay', () => {
  const grey = [204, 204, 204, 255]
  const sources = [
    windowWith('RaycastUIAccessHelperWindow', { isEmpty: () => true, toBitmap: () => Buffer.alloc(0) }),
    windowWith('NVIDIA GeForce Overlay', image([0, 0, 0, 255])),
    windowWith('Transparent helper', image([0, 0, 0, 0])),
    windowWith('Invisible but bright', image([255, 255, 255, 0])),
    windowWith('Near-black noise', image([10, 12, 14, 255])),
    windowWith('Command Prompt', image([12, 12, 12, 255], [[40, grey], [41, grey], [75, grey]])),
    { id: 'screen:0:0', name: 'Screen 1', thumbnail: image([0, 0, 0, 255]) },
  ]
  assert.deepEqual(visibleSources(sources).map(s => s.name), ['Command Prompt', 'Screen 1'])
})

// Resolves to 'pending' if the promise has not settled within a few ticks.
const settled = (promise, ms = 50) => Promise.race([promise, new Promise(r => setTimeout(() => r('pending'), ms))])

test('overlapping capture requests are each answered exactly once', async () => {
  const lists = [], shown = [], dismissed = []
  const p = picker({
    list: () => new Promise(r => lists.push(r)),
    show: id => shown.push(id), dismiss: id => dismissed.push(id), platform: 'win32',
  })
  const answers = [[], []]
  const first = p.request(true).then(a => answers[0].push(a))
  const second = p.request(true).then(a => answers[1].push(a))
  lists[0]([screen]); lists[1]([screen, editor])
  await settled(Promise.resolve())
  p.choose(shown[1], editor.id, false)
  assert.notEqual(await settled(first), 'pending', 'the older request was never answered')
  await second
  assert.deepEqual(answers, [[{}], [{ video: editor }]])
  assert.deepEqual(dismissed, [shown[0]])
})

test('a picker left open times out with {} and tells the renderer to close', async () => {
  const dismissed = []
  let shownId
  const p = picker({ list: async () => [screen], show: id => { shownId = id }, dismiss: id => dismissed.push(id), platform: 'win32', timeoutMs: 20 })
  assert.deepEqual(await p.request(true), {})
  assert.deepEqual(dismissed, [shownId])
  assert.equal(await p.refresh(), null, 'refresh after the timeout must report the picker closed')
  p.choose(shownId, screen.id, true)
})

test('a refresh reports closed once the picker was answered', async () => {
  let shownId
  const p = picker({ list: async () => [screen, editor], show: id => { shownId = id }, dismiss: () => {}, platform: 'linux' })
  const answer = p.request(false)
  await settled(Promise.resolve())
  assert.deepEqual((await p.refresh()).map(s => s.id), [screen.id, editor.id])
  p.choose(shownId, null, false)
  assert.deepEqual(await answer, {})
  assert.equal(await p.refresh(), null)
})
