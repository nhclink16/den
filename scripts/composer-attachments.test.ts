import { test } from 'node:test'
import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { consumedUploadIds } from '../apps/web/src/lib/composer.ts'

// Submitting `/canvas Review` with a completed attachment used to clear the
// composer's pending queue even though the canvas request never carries the
// upload ID, so the file silently never sent. A successful command must leave
// completed attachments queued; only an ordinary message send consumes them.

// Same removal rule as Uploads.sent in apps/web/src/lib/uploads.svelte.ts:
// an entry leaves the queue only when its completed upload ID was consumed.
type QueueEntry = { done?: { id: string } }
const applySent = (queue: QueueEntry[], ids: string[]) =>
  queue.filter((p) => !p.done || !ids.includes(p.done.id))

test('a successful slash command keeps completed attachments queued', () => {
  const queue: QueueEntry[] = [{ done: { id: 'upload-1' } }]
  const ready = ['upload-1']

  const consumed = consumedUploadIds(true, ready)
  assert.deepEqual(consumed, [], 'a command transmits no attachments')

  const kept = applySent(queue, consumed)
  assert.equal(kept.length, 1, 'the completed upload stays available to send')
})

test('an ordinary message send consumes the attachments it transmits', () => {
  const queue: QueueEntry[] = [{ done: { id: 'upload-1' } }]
  const ready = ['upload-1']

  const consumed = consumedUploadIds(false, ready)
  assert.deepEqual(consumed, ready, 'a message send transmits its completed uploads')

  const kept = applySent(queue, consumed)
  assert.equal(kept.length, 0, 'transmitted uploads leave the queue')
})

test('the composer routes its queue cleanup through consumedUploadIds', () => {
  const source = readFileSync('apps/web/src/ui/Composer.svelte', 'utf8')
  assert.match(source, /from '\.\.\/lib\/composer'/, 'Composer imports the composer helper')
  assert.match(source, /queue\.sent\(channelId, consumedUploadIds\(/, 'queue cleanup honors commands')
})
