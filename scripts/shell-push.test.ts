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
  const state = { permission, prompt: null as Promise<void> | null }
  const shell = {
    push: async (m: { type: string }) => {
      sent.push(m.type)
      if (m.type === 'register' && state.prompt) await state.prompt
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

test('sign-out takes the question down', async () => {
  const p = load('undecided')
  await p.pushStart({ askFirst: true })
  p.pushSignedOut()
  assert.equal(p.push.asking, false)
})
