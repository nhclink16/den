import { test } from 'node:test'
import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { matchCommand, runComposerSubmit, type ComposerCommand } from '../apps/web/src/lib/composer.ts'

// Issue #23: submitting `/canvas Review` with a completed attachment removed the
// attachment chip even though the canvas command never sends its upload ID. The
// server bytes survived; the user's unsent selection was silently lost.

// Mirrors the real /canvas plugin: it creates an object and refreshes messages,
// and never receives or forwards pending upload IDs.
const canvas = (seen: { args: string; calls: number }): ComposerCommand => ({
  name: 'canvas',
  run: async (ctx) => { seen.calls++; seen.args = ctx.args },
})

function spy() {
  const calls = { send: [] as { upload_ids: string[] }[], sent: [] as string[][], post: 0 }
  return {
    calls,
    send: async (_channel: string, _content: string, opts: { upload_ids: string[] }) => {
      calls.send.push({ upload_ids: opts.upload_ids })
    },
    post: async () => { calls.post++ },
    sent: (_channel: string, ids: string[]) => { calls.sent.push(ids) },
  }
}

test('a successful /canvas submission retains the completed attachment', async () => {
  const seen = { args: '', calls: 0 }
  const s = spy()
  const ready = ['upload-1']
  const out = await runComposerSubmit({
    channelId: 'room-1',
    content: '/canvas Review',
    ready,
    commands: [canvas(seen)],
    send: s.send,
    post: s.post,
    sent: s.sent,
  })
  assert.equal(out.handled, 'command')
  assert.equal(seen.calls, 1, 'the canvas command still runs')
  assert.equal(seen.args, 'Review')
  assert.equal(s.calls.send.length, 0, 'no message carries the upload ID')
  assert.equal(s.calls.sent.length, 0, 'the queue entry is not consumed')
  // The completed upload stays available to send next; nothing silently dropped it.
  const remaining = ready.filter((id) => !s.calls.sent.flat().includes(id))
  assert.deepEqual(remaining, ['upload-1'])
})

test('an ordinary message still sends its attachments and consumes them', async () => {
  const s = spy()
  const out = await runComposerSubmit({
    channelId: 'room-1',
    content: 'hello',
    ready: ['upload-1'],
    commands: [canvas({ args: '', calls: 0 })],
    send: s.send,
    post: s.post,
    sent: s.sent,
  })
  assert.equal(out.handled, 'message')
  assert.deepEqual(s.calls.send, [{ upload_ids: ['upload-1'] }])
  assert.deepEqual(s.calls.sent, [['upload-1']])
})

test('a failed command or a failed send leaves the queue untouched', async () => {
  const failing: ComposerCommand = { name: 'canvas', run: async () => { throw new Error('canvas down') } }
  const s1 = spy()
  await assert.rejects(() => runComposerSubmit({
    channelId: 'room-1', content: '/canvas Review', ready: ['upload-1'],
    commands: [failing], send: s1.send, post: s1.post, sent: s1.sent,
  }), /canvas down/)
  assert.equal(s1.calls.sent.length, 0)

  const s2 = spy()
  const badSend = async () => { throw new Error('send down') }
  await assert.rejects(() => runComposerSubmit({
    channelId: 'room-1', content: 'hello', ready: ['upload-1'],
    commands: [], send: badSend, post: s2.post, sent: s2.sent,
  }), /send down/)
  assert.equal(s2.calls.sent.length, 0)
})

test('command matching needs the full name: prefixes fall through to messages', () => {
  const commands = [{ name: 'canvas' }]
  assert.deepEqual(matchCommand('/canvas Review', commands), { name: 'canvas', args: 'Review' })
  assert.deepEqual(matchCommand('/canvas', commands), { name: 'canvas', args: '' })
  assert.equal(matchCommand('/canvass x', commands), null)
  assert.equal(matchCommand('/unknown x', commands), null)
})

// The composer owns the real queue, so pin its wiring: submit must go through
// the orchestrator above instead of unconditionally consuming the ready IDs
// after a command that never sent them.
test('Composer.svelte keeps command submissions out of the upload queue', () => {
  const source = readFileSync('apps/web/src/ui/Composer.svelte', 'utf8')
  assert.match(source, /runComposerSubmit/, 'submit must route through the tested orchestrator')
  assert.doesNotMatch(
    source,
    /queue\.sent\(channelId, ready\)/,
    'a successful command must not consume the ready upload IDs',
  )
})
