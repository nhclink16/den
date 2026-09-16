import { test } from 'node:test'
import assert from 'node:assert/strict'
import { planComposerSubmit, shouldClearDraft, type DraftIdentity } from '../apps/web/src/lib/composer-submit.ts'
import { room } from '../apps/web/src/lib/conversation.ts'

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

// Draft completion (#27). The composer stays editable while a send is in flight,
// so success must only clear the draft it actually submitted.
const draft = (over: Partial<DraftIdentity> = {}): DraftIdentity => ({ conversation: room('c1'), replyToId: null, revision: 3, ...over })

test('an untouched draft is cleared when its send succeeds', () => {
  assert.equal(shouldClearDraft(draft(), draft()), true)
})

test('an edit during the send keeps the newer draft', () => {
  assert.equal(shouldClearDraft(draft(), draft({ revision: 4 })), false)
})

test('editing away and back is still a new draft', () => {
  // A -> B -> A: the text matches again, but two edits happened. Comparing
  // strings instead of counting edits would erase what the person retyped.
  const submitted = draft({ revision: 3 })
  assert.equal(shouldClearDraft(submitted, draft({ revision: 5 })), false)
})

test('a reply target chosen during the send is not cleared', () => {
  assert.equal(shouldClearDraft(draft(), draft({ replyToId: 'm1' })), false)
  assert.equal(shouldClearDraft(draft({ replyToId: 'm1' }), draft({ replyToId: null })), false)
})

test('a completion never clears another channel', () => {
  assert.equal(shouldClearDraft(draft({ conversation: room('c1') }), draft({ conversation: room('c2') })), false)
})
