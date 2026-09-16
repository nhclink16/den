import { test } from 'node:test'
import assert from 'node:assert/strict'
import { conversationKey, room, sameConversation, type Conversation } from '../apps/web/src/lib/conversation.ts'
import { shouldClearDraft, planComposerSubmit } from '../apps/web/src/lib/composer-submit.ts'
import { append, busy, busyInChannel, consume, drop, patch, tray, type Queues } from '../apps/web/src/lib/upload-queue.ts'
import { Drafts, type Draft } from '../apps/web/src/lib/drafts.ts'

const c = (channelId: string, rootId: string | null): Conversation => ({ channelId, rootId })

// --- Conversation identity -------------------------------------------------

test('a room and a conversation inside it are different identities', () => {
  assert.ok(!sameConversation(room('ch'), c('ch', 'root-a')))
  assert.ok(!sameConversation(c('ch', 'root-a'), c('ch', 'root-b')))
  assert.ok(sameConversation(room('ch'), c('ch', null)))
  // Keys cannot collide: a room key is never a conversation key in that channel.
  assert.notEqual(conversationKey(room('ch')), conversationKey(c('ch', 'root-a')))
  assert.notEqual(conversationKey(c('ch', 'a')), conversationKey(c('ch', 'b')))
  assert.notEqual(conversationKey(c('ch', 'a')), conversationKey(c('other', 'a')))
})

// --- Draft ownership (#27's rule, now per conversation) --------------------

test('a completion clears only the draft it was submitted from', () => {
  // Both conversations sit at revision 1 with no reply target, which is exactly
  // the case channel-only identity could not tell apart.
  const a = { conversation: c('ch', 'root-a'), replyToId: null, revision: 1 }
  const b = { conversation: c('ch', 'root-b'), replyToId: null, revision: 1 }
  assert.ok(shouldClearDraft(a, a))
  assert.ok(!shouldClearDraft(a, b), 'a send in A cleared B')
  assert.ok(!shouldClearDraft(a, { ...a, conversation: room('ch') }), 'a send in A cleared the room draft')
  assert.ok(!shouldClearDraft({ ...a, conversation: room('ch') }, a), 'a send in the room cleared A')
})

test('typing A then B then A during a send is still a newer draft', () => {
  const submitted = { conversation: room('ch'), replyToId: null, revision: 4 }
  // The text matches what was submitted, but two edits happened meanwhile.
  assert.ok(!shouldClearDraft(submitted, { ...submitted, revision: 6 }))
  assert.ok(shouldClearDraft(submitted, { ...submitted, revision: 4 }))
})

test('a reply target chosen during the send keeps its own draft', () => {
  const submitted = { conversation: c('ch', 'root-a'), replyToId: 'm1', revision: 2 }
  assert.ok(!shouldClearDraft(submitted, { ...submitted, replyToId: 'm2' }))
  assert.ok(!shouldClearDraft(submitted, { ...submitted, replyToId: null }))
})

test('a thread id arriving mid-send does not move the draft or its tray', () => {
  // The root is the handle before and after the server creates the thread. The
  // failure this guards is keying on the thread: it does not exist when the
  // first reply is submitted, so adopting it on the response would strand both
  // the draft that is still open and the files staged under it.
  const root = 'root-a'
  const threadTheServerAssigned = 'thread-77'
  const before = { conversation: c('ch', root), replyToId: root, revision: 3 }

  let queues: Queues<{ id: number; done?: { id: string } }> = {}
  queues = append(queues, conversationKey(before.conversation), { id: 1, done: { id: 'up-staged-after' } })

  const after = { conversation: c('ch', root), replyToId: root, revision: 3 }
  assert.ok(shouldClearDraft(before, after))
  assert.equal(tray(queues, conversationKey(after.conversation)).length, 1)
  // Had the identity followed the server, both would have moved to a key that
  // holds nothing.
  assert.notEqual(conversationKey(after.conversation), conversationKey(c('ch', threadTheServerAssigned)))
  assert.equal(tray(queues, conversationKey(c('ch', threadTheServerAssigned))).length, 0)
})

// --- Upload trays ----------------------------------------------------------

type Staged = { id: number; done?: { id: string }; error?: string }
const roomKey = conversationKey(room('ch'))
const aKey = conversationKey(c('ch', 'root-a'))

test('a pending upload in one conversation neither appears in nor blocks another', () => {
  let q: Queues<Staged> = {}
  q = append(q, roomKey, { id: 1 })
  q = append(q, aKey, { id: 2, done: { id: 'up-a' } })

  assert.deepEqual(tray(q, aKey).map((p) => p.id), [2], 'the room file leaked into the conversation')
  assert.ok(busy(tray(q, roomKey)), 'the room upload is still running')
  assert.ok(!busy(tray(q, aKey)), "the room's upload blocked sending in the conversation")
  // The sidebar still reports the channel as busy from either tray.
  assert.ok(busyInChannel(q, 'ch'))
  assert.ok(!busyInChannel(q, 'other'))
})

test('progress and completion reach only their own captured tray', () => {
  let q: Queues<Staged> = {}
  q = append(q, roomKey, { id: 1 })
  q = append(q, aKey, { id: 1 })
  // Ids are per queue owner, so the same id exists in both trays; a callback
  // must be addressed by key as well.
  q = patch(q, aKey, 1, { done: { id: 'up-a' } })
  assert.equal(tray(q, aKey)[0].done?.id, 'up-a')
  assert.equal(tray(q, roomKey)[0].done, undefined, "a completion wrote into another conversation's tray")

  q = patch(q, roomKey, 1, { error: 'Upload failed' })
  assert.equal(tray(q, aKey)[0].error, undefined)
  assert.ok(!busyInChannel(q, 'ch'), 'one done and one failed leaves nothing running')
})

test('a successful send consumes exactly what it transmitted', () => {
  let q: Queues<Staged> = {}
  q = append(q, aKey, { id: 1, done: { id: 'up-1' } })
  q = append(q, aKey, { id: 2, done: { id: 'up-2' } })
  q = append(q, roomKey, { id: 3, done: { id: 'up-3' } })

  // Only up-1 went out; up-2 was staged while the send was in flight.
  q = consume(q, aKey, ['up-1'])
  assert.deepEqual(tray(q, aKey).map((p) => p.done?.id), ['up-2'], 'a newer staged file was eaten')
  assert.deepEqual(tray(q, roomKey).map((p) => p.done?.id), ['up-3'], 'the room tray was consumed too')

  // #29: a slash command transmits nothing, so it consumes nothing.
  const before = tray(q, roomKey).length
  q = consume(q, roomKey, [])
  assert.equal(tray(q, roomKey).length, before)

  // Consumption is addressed by tray, not by id alone: naming A's remaining
  // upload while consuming the room's tray must leave A untouched.
  q = consume(q, roomKey, ['up-2'])
  assert.deepEqual(tray(q, aKey).map((p) => p.done?.id), ['up-2'], 'a send elsewhere consumed this tray')
})

test('an emptied tray is removed rather than left behind', () => {
  let q: Queues<Staged> = {}
  q = append(q, aKey, { id: 1, done: { id: 'up-1' } })
  q = consume(q, aKey, ['up-1'])
  assert.deepEqual(Object.keys(q), [])
  q = append(q, roomKey, { id: 2 })
  q = drop(q, roomKey, 2)
  assert.deepEqual(Object.keys(q), [])
})

// --- Unchanged send planning ----------------------------------------------

test('slash commands still never consume attachments they did not send', () => {
  assert.deepEqual(planComposerSubmit('/canvas Board', ['up1'], ['canvas']), {
    kind: 'command', name: 'canvas', args: 'Board', uploadIds: [],
  })
  assert.deepEqual(planComposerSubmit('hello', ['up1'], ['canvas']), {
    kind: 'message', uploadIds: ['up1'],
  })
  assert.equal(planComposerSubmit('   ', [], ['canvas']), null)
})

// --- The real draft owner ---------------------------------------------------
// Identity tests above build revisions by hand. These drive the owner the
// composer actually uses, over the same plain cell the Store gives it.

function drafts() {
  let entries: Record<string, Draft> = {}
  return new Drafts({ get: () => entries, set: (v) => { entries = v } })
}

/// Exactly what Composer.submit decides with, so the test cannot agree with a
/// broken implementation by re-deriving the rule.
const identity = (owner: Drafts, c: Conversation) =>
  ({ conversation: c, replyToId: owner.for(c).replyToId, revision: owner.for(c).revision })
const wouldClear = (owner: Drafts, token: number, submitted: ReturnType<typeof identity>, c: Conversation) =>
  owner.holds(token) && shouldClearDraft(submitted, identity(owner, c))

test('the owner counts edits, not text states', () => {
  const owner = drafts()
  const here = room('ch')
  owner.setText(here, 'a')
  owner.setText(here, 'a')          // a re-render is not an edit
  assert.equal(owner.for(here).revision, 1)
  owner.setText(here, 'b')
  owner.setText(here, 'a')          // A -> B -> A is two more edits
  assert.equal(owner.for(here).revision, 3)
  assert.equal(owner.for(here).text, 'a')
  // Changing the quote is part of the draft but is not a text edit.
  owner.setReplyTo(here, 'm1')
  assert.equal(owner.for(here).revision, 3)
  assert.equal(owner.for(here).replyToId, 'm1')
})

test('a send in flight across a clear must not clear the next account draft', () => {
  // clear() restarts revisions, so revision alone is not identity. Without the
  // token this is a draft with the same room, no quote and revision 1 — exactly
  // what the earlier send captured.
  const owner = drafts()
  const here = room('ch')
  owner.setText(here, 'mine')
  const token = owner.token
  const submitted = identity(owner, here)
  assert.equal(submitted.revision, 1)

  owner.clear()
  owner.setText(here, 'someone else typing after logout')
  assert.equal(owner.for(here).revision, 1, 'this test is only meaningful while the revisions collide')
  assert.deepEqual(identity(owner, here), { ...submitted, revision: 1 })

  assert.equal(wouldClear(owner, token, submitted, here), false, "a send cleared the next account's draft")
  assert.equal(owner.for(here).text, 'someone else typing after logout')
})

test('two owners with the same conversation ids stay separate', () => {
  // Two instances, same room id. A completion belongs to the owner it read from.
  const a = drafts(), b = drafts()
  const here = room('ch')
  a.setText(here, 'instance a')
  b.setText(here, 'instance b')
  const token = a.token
  const submitted = identity(a, here)

  assert.equal(wouldClear(b, token, submitted, here), true, 'the fixture is not comparable')
  // Which is why the composer compares against the CAPTURED owner, never the
  // active one: the same token and revision exist in both.
  a.setText(here, 'edited')
  assert.equal(wouldClear(a, token, submitted, here), false)
  b.setText(here, 'b keeps typing')
  assert.equal(b.for(here).text, 'b keeps typing')
  assert.equal(a.for(here).text, 'edited')
})

test('clearing empties every conversation and invalidates the old token', () => {
  const owner = drafts()
  const first = room('ch'), second = c('ch', 'root-a')
  owner.setText(first, 'room draft')
  owner.setText(second, 'conversation draft')
  const token = owner.token
  owner.clear()
  assert.equal(owner.for(first).text, '')
  assert.equal(owner.for(second).text, '')
  assert.equal(owner.holds(token), false)
  assert.equal(owner.holds(owner.token), true)
})

test('a quote survives edits and only an explicit clear removes it', () => {
  const owner = drafts()
  const here = c('ch', 'root-a')
  owner.setReplyTo(here, 'older-than-the-page')
  owner.setText(here, 'answering')
  owner.setText(here, 'answering at length')
  assert.equal(owner.for(here).replyToId, 'older-than-the-page',
    'the quote was lost while the draft was edited')
  owner.setReplyTo(here, null)
  assert.equal(owner.for(here).replyToId, null)
})
