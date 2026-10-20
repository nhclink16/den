import { test } from 'node:test'
import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { createRequire } from 'node:module'
import { runInNewContext } from 'node:vm'

// Den Shell's notification question. iOS shows its own prompt only once, so Den asks first in its own words and
// goes to iOS only on Turn On. What must hold: nothing reaches iOS's prompt before a yes, a phone that already
// answered registers at every launch as before, and Not Now sticks. This runs the real shell-push.svelte.ts with
// its imports stubbed and a fake shell that records what the page asked for.

const require_ = createRequire(import.meta.url)
const ts = require_('../apps/web/node_modules/typescript')
const source = readFileSync(new URL('../apps/web/src/lib/shell-push.svelte.ts', import.meta.url), 'utf8').replace(/^import .*$/gm, '')

function load(permission: 'granted' | 'denied' | 'undecided') {
  const sent: string[] = []
  const stored = new Map<string, string>()
  // `prompt`: while set, iOS's prompt is up and a register waits for it.
  // `status`: while set, a status reply waits for it (read when it resolves).
  const state = { permission, prompt: null as Promise<void> | null, status: null as Promise<void> | null }
  const shell = {
    push: async (m: { type: string }) => {
      sent.push(m.type)
      if (m.type === 'register' && state.prompt) await state.prompt
      if (m.type === 'status' && state.status) await state.status
      if (m.type === 'register' && state.permission === 'undecided') state.permission = 'granted'
      if (m.type === 'register' || m.type === 'status') return { permission: state.permission, environment: 'sandbox', appVersion: 'test' }
      return null
    },
  }
  const exports: Record<string, any> = {}
  const js = ts.transpileModule(source, { compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022 } }).outputText
  runInNewContext(js, {
    exports, Promise, shell, $state: (x: unknown) => x,
    instances: { active: { ready: false } },
    localStorage: { getItem: (k: string) => stored.get(k) ?? null, setItem: (k: string, v: string) => stored.set(k, v), removeItem: (k: string) => stored.delete(k) },
  })
  return { ...exports, sent, state } as any
}
const settle = () => new Promise((r) => setTimeout(r, 0))

test('after sign-in, an iPhone iOS has not asked yet gets Den\'s question, and nothing goes to iOS', async () => {
  const p = load('undecided')
  await p.pushStart({ askFirst: true })
  assert.equal(p.push.asking, true)
  assert.deepEqual(p.sent, ['status'], 'no register, so no iOS prompt')
})

test('Turn On registers, which brings up iOS\'s prompt, and Den coming back meanwhile does not ask again', async () => {
  const p = load('undecided')
  await p.pushStart({ askFirst: true })
  let tap!: () => void
  p.state.prompt = new Promise<void>((r) => { tap = r })
  p.pushAnswer(true)
  assert.equal(p.push.asking, false)
  await settle()
  assert.equal(p.sent.at(-1), 'register')
  // Den comes back to the front while iOS's prompt is up: the shell still says undecided.
  await p.pushStart({ askFirst: true })
  assert.equal(p.push.asking, false, 'asked again on top of the prompt')
  tap(); await settle(); await settle()
  assert.equal(p.state.permission, 'granted')
})

test('Not Now sticks for this iPhone, and Settings\' Turn on still goes to iOS', async () => {
  const p = load('undecided')
  await p.pushStart({ askFirst: true })
  p.pushAnswer(false)
  assert.equal(p.push.asking, false)
  await p.pushStart({ askFirst: true })
  assert.equal(p.push.asking, false)
  assert.ok(!p.sent.includes('register'))
  await p.pushStart()
  assert.ok(p.sent.includes('register'))
})

test('an iPhone that already answered registers at every launch, without a question', async () => {
  for (const permission of ['granted', 'denied'] as const) {
    const p = load(permission)
    await p.pushStart({ askFirst: true })
    assert.equal(p.push.asking, false)
    assert.deepEqual(p.sent.slice(0, 2), ['status', 'register'], permission)
  }
})

test('a check that replies after sign-out neither registers nor asks', async () => {
  for (const permission of ['granted', 'undecided'] as const) {
    const p = load(permission)
    let reply!: () => void
    p.state.status = new Promise<void>((r) => { reply = r })
    const started = p.pushStart({ askFirst: true })
    p.pushSignedOut()
    reply(); await started
    assert.ok(!p.sent.includes('register'), permission)
    assert.equal(p.push.asking, false, permission)
  }
})

test('an "undecided" that replies after iOS\'s prompt was answered does not bring the question back', async () => {
  const p = load('undecided')
  await p.pushStart({ askFirst: true })
  let tap!: () => void, reply!: () => void
  p.state.prompt = new Promise<void>((r) => { tap = r })
  p.pushAnswer(true)
  await settle()
  // Den comes back while the prompt is up; the shell's answer is slow and says undecided.
  p.state.status = new Promise<void>((r) => { reply = r })
  const late = p.pushStart({ askFirst: true })
  tap(); await settle(); await settle()
  p.state.permission = 'undecided'
  reply(); await late
  assert.equal(p.push.asking, false)
})

test('an old account\'s Turn On finishing after sign-out does not cancel the next account\'s registration', async () => {
  const p = load('undecided')
  await p.pushStart({ askFirst: true })
  let tap!: () => void
  p.state.prompt = new Promise<void>((r) => { tap = r })
  p.pushAnswer(true)
  await settle()
  p.pushSignedOut()
  // The next account signs in while iOS's prompt is still up; its launch check is slow.
  let reply!: () => void
  p.state.status = new Promise<void>((r) => { reply = r })
  const next = p.pushStart({ askFirst: true })
  tap(); await settle(); await settle()
  p.state.prompt = null
  reply(); await next
  assert.equal(p.sent.filter((t: string) => t === 'register').length, 2, 'the next account registered')
})

test('Settings\' Turn on is protected the same way: a stale "undecided" does not bring the question back', async () => {
  const p = load('undecided')
  let tap!: () => void, reply!: () => void
  p.state.prompt = new Promise<void>((r) => { tap = r })
  const settings = p.pushStart()
  await settle()
  p.state.status = new Promise<void>((r) => { reply = r })
  const late = p.pushStart({ askFirst: true })
  tap(); await settings
  p.state.permission = 'undecided'
  reply(); await late
  assert.equal(p.push.asking, false)
})

test('sign-out takes the question down', async () => {
  const p = load('undecided')
  await p.pushStart({ askFirst: true })
  p.pushSignedOut()
  assert.equal(p.push.asking, false)
})
