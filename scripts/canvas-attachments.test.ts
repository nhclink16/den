import { test } from 'node:test'
import assert from 'node:assert/strict'
import { planComposerSubmit } from '../apps/web/src/lib/composer-submit.ts'

// Mirrors Uploads.sent in apps/web/src/lib/uploads.svelte.ts: it drops only the
// completed IDs it is given, keeping everything else queued.
function remainingAfterSent(completed: string[], consumed: string[]) {
  return completed.filter((id) => !consumed.includes(id))
}

test('successful /canvas submission retains completed attachments', () => {
  const plan = planComposerSubmit('/canvas Review', ['upload-1'], ['canvas', 'terminal'])
  assert.equal(plan?.kind, 'command')
  assert.deepEqual(plan?.uploadIds, [])
  assert.deepEqual(remainingAfterSent(['upload-1'], plan!.uploadIds), ['upload-1'])
})

test('bare /canvas retains attachments too', () => {
  const plan = planComposerSubmit('/canvas', ['upload-1'], ['canvas', 'terminal'])
  assert.equal(plan?.kind, 'command')
  assert.deepEqual(remainingAfterSent(['upload-1'], plan!.uploadIds), ['upload-1'])
})

test('ordinary message consumes completed attachments', () => {
  const plan = planComposerSubmit('hello', ['upload-1'], ['canvas', 'terminal'])
  assert.equal(plan?.kind, 'message')
  assert.deepEqual(plan?.uploadIds, ['upload-1'])
  assert.deepEqual(remainingAfterSent(['upload-1'], plan!.uploadIds), [])
})

test('attachment-only send consumes, /canvasfoo stays a message', () => {
  const only = planComposerSubmit('', ['upload-1'], ['canvas'])
  assert.equal(only?.kind, 'message')
  assert.deepEqual(only?.uploadIds, ['upload-1'])

  const prefix = planComposerSubmit('/canvasfoo', ['upload-1'], ['canvas'])
  assert.equal(prefix?.kind, 'message')
  assert.deepEqual(prefix?.uploadIds, ['upload-1'])
})

test('empty composer with nothing queued plans nothing', () => {
  assert.equal(planComposerSubmit('', [], ['canvas']), null)
  assert.equal(planComposerSubmit('   ', [], ['canvas']), null)
})
