import { test } from 'node:test'
import assert from 'node:assert/strict'
import { openStrip, pickShellPhotos, readPhoto, stripSelected, type ShellPhoto, type ShellStripEvent, type StripTray } from '../apps/web/src/lib/shell-media.ts'

// Den Shell hands a picked photo or video to the page in base64 pieces; the page must put it back together whole,
// under its own name and type, and one bad file in a pick mustn't stop the rest.
const PIECE = 5
const shellFiles: Record<string, Buffer> = {}
const chunk = async (id: string, offset: number) => {
  const file = shellFiles[id]
  if (!file) throw new Error('That file is gone.')
  return file.subarray(offset, offset + PIECE).toString('base64')
}
const photo = (id: string, bytes: Buffer, name = `${id}.jpeg`): ShellPhoto => {
  shellFiles[id] = bytes
  return { id, name, type: 'image/jpeg', size: bytes.length }
}

test('a file read in pieces comes back whole, with its name and type', async () => {
  const bytes = Buffer.from(Array.from({ length: 23 }, (_, i) => (i * 37) & 255))
  const asked: number[] = []
  const file = await readPhoto(photo('a', bytes, 'IMG_0111.jpeg'), (id, offset) => { asked.push(offset); return chunk(id, offset) })
  assert.deepEqual(Buffer.from(await file.arrayBuffer()), bytes)
  assert.equal(file.name, 'IMG_0111.jpeg')
  assert.equal(file.type, 'image/jpeg')
  assert.deepEqual(asked, [0, 5, 10, 15, 20])
})

test('an empty file is one ask, and a file shorter than promised fails instead of looping', async () => {
  assert.equal((await readPhoto(photo('empty', Buffer.alloc(0)), chunk)).size, 0)
  const short = { ...photo('short', Buffer.from('abc')), size: 10 }
  await assert.rejects(readPhoto(short, chunk), /short read/)
})

test('a pick adds each file in order, skips one over the limit, keeps going past one that fails, and says what was lost', async () => {
  const picked = [photo('one', Buffer.from('first')), { ...photo('huge', Buffer.from('x')), size: 1024 ** 3 + 1 },
    { id: 'gone', name: 'gone.mov', type: 'video/quicktime', size: 4 }, photo('two', Buffer.from('second'))]
  ;(globalThis as { window?: unknown }).window = { denShell: { photos: async () => ({ files: picked, failed: 1 }), photoChunk: chunk } }
  const added: string[] = [], warned: string[] = []
  await pickShellPhotos((files) => added.push(...files.map((f) => f.name)), (m) => warned.push(m))
  assert.deepEqual(added, ['one.jpeg', 'two.jpeg'])
  assert.deepEqual(warned, ["One of those couldn't be added.", 'huge.jpeg is over the 1 GB limit.', "gone.mov couldn't be added."])
})

test('a picker that fails to open says why', async () => {
  ;(globalThis as { window?: unknown }).window = { denShell: { photos: async () => { throw new Error("Den can't show your photos right now.") } } }
  const warned: string[] = []
  await pickShellPhotos(() => assert.fail('nothing to add'), (m) => warned.push(m))
  assert.deepEqual(warned, ["Den can't show your photos right now."])
})

// The + sheet's strip of recent photos: a tick puts the photo in the tray, an untick takes it out (or drops it if it is
// still on its way), and a new strip ticks the strip's photos that are in the tray or still coming.
function stripRig(key = 'room') {
  const holds: Record<string, () => void> = {}
  const slow = async (id: string, offset: number) => { if (id.startsWith('late')) await new Promise<void>((r) => (holds[id] = r)); return chunk(id, offset) }
  const shell: { photoChunk: typeof chunk; onPhotoStrip?: ((e: ShellStripEvent) => void) | null } = { photoChunk: slow }
  ;(globalThis as { window?: unknown }).window = { denShell: shell }
  const files: File[] = [], warned: string[] = []
  const tray: StripTray = { key, files: () => files, add: (f) => files.push(f), remove: (f) => files.splice(files.indexOf(f), 1), warn: (m) => warned.push(m) }
  const send = (e: ShellStripEvent) => shell.onPhotoStrip!(e)
  const pick = (session: number, asset: string, tick: string, id = asset) => {
    send({ type: 'ticked', session, asset, tick })
    send({ type: 'added', session, asset, tick, file: photo(id, Buffer.from(id)) })
  }
  return { tray, files, warned, send, pick, holds, names: () => files.map((f) => f.name) }
}
const settle = () => new Promise((r) => setTimeout(r, 0))

test('the strip fills and empties the tray it was opened from', async () => {
  const { tray, warned, send, pick, holds, names } = stripRig()
  const session = openStrip(tray)
  pick(session, 'A', 't1', 'lake')
  pick(session, 'B', 't2', 'late-b')
  await settle()
  assert.deepEqual(names(), ['lake.jpeg'])
  assert.deepEqual(stripSelected(tray), ['A', 'B'])

  // Unticked while it was still coming in: it never reaches the tray.
  send({ type: 'removed', session, asset: 'B' })
  holds['late-b']!()
  await settle()
  assert.deepEqual(names(), ['lake.jpeg'])

  send({ type: 'removed', session, asset: 'A' })
  assert.deepEqual(names(), [])
  send({ type: 'ticked', session, asset: 'C', tick: 't3' })
  send({ type: 'failed', session, asset: 'C', tick: 't3' })
  // A strip the page never opened doesn't land anything.
  pick(session + 1, 'D', 't4', 'other')
  await settle()
  assert.deepEqual(names(), [])
  assert.deepEqual(stripSelected(tray), [])
  assert.deepEqual(warned, ["That photo couldn't be added."])
})

test('a photo still coming when the sheet opens again shows ticked, lands once, and stays out once unticked', async () => {
  const { tray, send, holds, names } = stripRig('dm')
  const first = openStrip(tray)
  send({ type: 'ticked', session: first, asset: 'A', tick: 't1' })
  const second = openStrip(tray)
  assert.deepEqual(stripSelected(tray), ['A'])
  // Unticked in the new strip before the first one's copy arrived: that copy is dropped.
  send({ type: 'removed', session: second, asset: 'A' })
  send({ type: 'added', session: first, asset: 'A', tick: 't1', file: photo('a1', Buffer.from('a')) })
  await settle()
  assert.deepEqual(names(), [])
  // Ticked again: only the newest tick lands, however the copies are ordered.
  send({ type: 'ticked', session: second, asset: 'A', tick: 't2' })
  send({ type: 'added', session: second, asset: 'A', tick: 't2', file: photo('late-a2', Buffer.from('a')) })
  send({ type: 'added', session: first, asset: 'A', tick: 't1', file: photo('a3', Buffer.from('a')) })
  holds['late-a2']!()
  await settle()
  assert.deepEqual(names(), ['late-a2.jpeg'])
  // Another conversation's strip doesn't see it as ticked.
  assert.deepEqual(stripSelected({ ...tray, key: 'room', files: () => [] }), [])
})
