import { test } from 'node:test'
import assert from 'node:assert/strict'
import { pickShellPhotos, readPhoto, type ShellPhoto } from '../apps/web/src/lib/shell-media.ts'

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
