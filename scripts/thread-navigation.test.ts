import { test } from 'node:test'
import assert from 'node:assert/strict'
import { conversationKey, room, sameConversation } from '../apps/web/src/lib/conversation.ts'
import { Drafts, type Draft } from '../apps/web/src/lib/drafts.ts'
import { shouldClearDraft } from '../apps/web/src/lib/composer-submit.ts'
import { append, busy, consume, tray, type Queues } from '../apps/web/src/lib/upload-queue.ts'
import { parse } from '../apps/web/src/lib/routes.ts'
import { ReadState } from '../apps/web/src/lib/read-state.ts'
import type { ThreadSummary } from '../apps/web/src/lib/types.ts'

const CH = 'ch1'
const conv = (rootId: string) => ({ channelId: CH, rootId })

function drafts() {
  let entries: Record<string, Draft> = {}
  return new Drafts({ get: () => entries, set: (v) => { entries = v } })
}

test('a conversation keeps its root identity when the server creates its thread', () => {
  // The panel opens on a root before any thread exists. The server then assigns
  // a thread ID, which is placement context only: if it became the identity, the
  // draft and files staged while typing the first reply would be stranded under
  // a key nothing is looking at.
  const owner = drafts()
  const here = conv('root-a')
  owner.setText(here, 'the first reply, still being typed')
  const token = owner.token
  const submitted = { conversation: here, replyToId: 'root-a', revision: owner.for(here).revision }

  let files: Queues<{ id: number; done?: { id: string } }> = {}
  files = append(files, conversationKey(here), { id: 1, done: { id: 'up-1' } })

  // The response arrives: thread-99 now exists for this root.
  const afterCreation = conv('root-a')
  assert.ok(sameConversation(here, afterCreation), 'the identity moved when the thread was created')
  assert.equal(conversationKey(afterCreation), conversationKey(here))
  assert.equal(owner.for(afterCreation).text, 'the first reply, still being typed')
  assert.equal(tray(files, conversationKey(afterCreation)).length, 1, 'the staged file was stranded')
  // And a key built from the server's ID would hold nothing at all.
  assert.equal(tray(files, conversationKey(conv('thread-99'))).length, 0)
  assert.ok(owner.holds(token) && shouldClearDraft(submitted, {
    conversation: afterCreation, replyToId: 'root-a', revision: owner.for(afterCreation).revision,
  }))
})

test('the room and two conversations in it keep separate drafts and files', () => {
  const owner = drafts()
  const main = room(CH), a = conv('root-a'), b = conv('root-b')
  owner.setText(main, 'something for the room')
  owner.setText(a, 'answering A')
  owner.setText(b, 'answering B')

  let files: Queues<{ id: number; done?: { id: string }; error?: string }> = {}
  files = append(files, conversationKey(main), { id: 1 })
  files = append(files, conversationKey(a), { id: 2, done: { id: 'up-a' } })

  assert.equal(owner.for(main).text, 'something for the room')
  assert.equal(owner.for(a).text, 'answering A')
  assert.equal(owner.for(b).text, 'answering B')
  // A file staged in the room does not block sending in a conversation.
  assert.ok(busy(tray(files, conversationKey(main))))
  assert.ok(!busy(tray(files, conversationKey(a))))
  assert.equal(tray(files, conversationKey(b)).length, 0)

  // A send completing in A clears only A, and consumes only what it sent.
  const submittedA = { conversation: a, replyToId: null, revision: owner.for(a).revision }
  assert.ok(!shouldClearDraft(submittedA, { conversation: b, replyToId: null, revision: owner.for(b).revision }))
  files = consume(files, conversationKey(a), ['up-a'])
  assert.equal(tray(files, conversationKey(a)).length, 0)
  assert.equal(tray(files, conversationKey(main)).length, 1, "a send in a conversation consumed the room's file")
  assert.equal(owner.for(b).text, 'answering B')
})

test('editing away and back in one conversation is still a newer draft', () => {
  const owner = drafts()
  const a = conv('root-a')
  owner.setText(a, 'first')
  const submitted = { conversation: a, replyToId: null, revision: owner.for(a).revision }
  owner.setText(a, 'second')
  owner.setText(a, 'first')
  assert.ok(!shouldClearDraft(submitted, { conversation: a, replyToId: null, revision: owner.for(a).revision }),
    'two edits back to the same text cleared the box')
})

// --- Saved URLs -------------------------------------------------------------

test('the canonical routes survive being pasted back in', () => {
  assert.deepEqual(parse('/c/AAA'), { name: 'channel', id: 'AAA', thread: undefined, message: undefined, reply: undefined })
  assert.deepEqual(parse('/c/AAA', '?m=MSG'),
    { name: 'channel', id: 'AAA', thread: undefined, message: 'MSG', reply: undefined })
  assert.deepEqual(parse('/c/AAA/t/TTT'),
    { name: 'channel', id: 'AAA', thread: 'TTT', message: undefined, reply: undefined })
  assert.deepEqual(parse('/c/AAA/t/TTT', '?m=MSG'),
    { name: 'channel', id: 'AAA', thread: 'TTT', message: 'MSG', reply: undefined })
  // Before the server has a thread, the conversation is named by its root.
  assert.deepEqual(parse('/c/AAA', '?reply=ROOT'),
    { name: 'channel', id: 'AAA', thread: undefined, message: undefined, reply: 'ROOT' })
})

test('routes that are not conversations are unchanged', () => {
  assert.deepEqual(parse('/inbox'), { name: 'inbox' })
  assert.deepEqual(parse('/find', '?q=deploy&in=AAA'), { name: 'search', q: 'deploy', channel: 'AAA' })
  assert.deepEqual(parse('/settings/account'), { name: 'settings', section: 'account' })
  assert.deepEqual(parse('/c/AAA/t/TTT/extra'), { name: 'home' }, 'a malformed thread URL is not a channel')
})

// --- Metadata ordering ------------------------------------------------------

test('a delayed list page cannot resurrect a conversation that was resolved', () => {
  // The exact hazard: an open-threads page is in flight, someone resolves the
  // conversation, and the page arrives still describing it as open.
  let meta = new Map<string, ThreadSummary>()
  const metas = new ReadState<ThreadSummary>({ get: () => meta, set: (v) => { meta = v } }, (t) => t.id)
  const open = (id: string, resolved = false): ThreadSummary => ({
    id, channel_id: CH, root_message_id: `root-${id}`, title: id, created_by: 'u1',
    created_at: '2026-01-01T00:00:00Z', last_activity_at: '2026-03-01T10:00:00Z', reply_count: 2,
    resolved_at: resolved ? '2026-03-01T11:00:00Z' : null, resolved_by: resolved ? 'u1' : null,
  })

  metas.apply(open('t1'))
  const epoch = metas.epoch
  const versions = metas.snapshot()      // the open-list request goes out here
  metas.apply(open('t1', true))          // socket: someone resolved it

  return metas.applyPage([open('t1'), open('t2')], epoch, versions).then(() => {
    assert.ok(meta.get('t1')?.resolved_at, 'a stale open list reopened a resolved conversation')
    assert.ok(meta.get('t2'), 'the rest of the page was dropped instead of applied per key')
  })
})
