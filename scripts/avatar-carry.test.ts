import { test } from 'node:test'
import assert from 'node:assert/strict'
import { carryOldAvatarChoices, forgetOldAvatarChoice, oldAvatarChoices } from '../apps/web/src/lib/avatar-carry.ts'

const server = (avatar_shape = 'theme', presence_style = 'dot') => ({ avatar_shape, presence_style }) as const
const layout = (extra: object = {}) => ({ sidebar: true, members: true, sounds: false, collapsed: [], ...extra })

test("this device's old choice goes up while the server still has the default", () => {
  assert.deepEqual(oldAvatarChoices(layout({ avatarShape: 'round', presence: 'ring' }), server()), { avatar_shape: 'round', presence_style: 'ring' })
})

test('a choice the server already has wins, field by field', () => {
  assert.deepEqual(oldAvatarChoices(layout({ avatarShape: 'round', presence: 'ring' }), server('soft')), { presence_style: 'ring' })
  assert.deepEqual(oldAvatarChoices(layout({ avatarShape: 'round', presence: 'ring' }), server('soft', 'off')), {})
})

test('an old copy holding the defaults, or junk, sends nothing but can go', () => {
  assert.deepEqual(oldAvatarChoices(layout({ avatarShape: 'theme', presence: 'dot' }), server()), {})
  assert.deepEqual(oldAvatarChoices(layout({ avatarShape: 'hexagon', presence: 7 }), server()), {})
})

test('nothing to do once the old copy is gone, or against a server that predates the fields', () => {
  assert.equal(oldAvatarChoices(layout(), server()), null)
  assert.equal(oldAvatarChoices(layout({ avatarShape: 'round' }), {}), null)
})

const storage = (start: object) => {
  const items = new Map([['den.layout', JSON.stringify(start)]])
  return { getItem: (k: string) => items.get(k) ?? null, setItem: (k: string, v: string) => { items.set(k, v) }, layout: () => JSON.parse(items.get('den.layout')!) }
}
const held = () => { let done!: (ok: boolean) => void; const p = new Promise<boolean>((r) => { done = r }); return { p, done } }
const old = layout({ avatarShape: 'round', presence: 'ring' })

test('the old copy stays on disk until the save works, and goes once it has', async () => {
  const s = storage(old), save = held(), sent: object[] = []
  const carrying = carryOldAvatarChoices(s, server(), (patch) => { sent.push(patch); return save.p })
  assert.deepEqual(s.layout(), old) // a reload now finds it and tries again
  // A second sync, or another server's Store, while the first save is on its way: nothing more goes.
  await carryOldAvatarChoices(s, server(), (patch) => { sent.push(patch); return Promise.resolve(true) })
  save.done(true); await carrying
  assert.deepEqual(sent, [{ avatar_shape: 'round', presence_style: 'ring' }])
  assert.deepEqual(s.layout(), layout())
})

test('a failed save keeps the old copy for the next sync', async () => {
  const s = storage(old)
  await carryOldAvatarChoices(s, server(), async () => false)
  assert.deepEqual(s.layout(), old)
  const sent: object[] = []
  await carryOldAvatarChoices(s, server(), async (patch) => { sent.push(patch); return true })
  assert.deepEqual(sent, [{ avatar_shape: 'round', presence_style: 'ring' }])
})

test('a choice made since wins over the old copy, field by field', async () => {
  const s = storage(old), sent: object[] = []
  await carryOldAvatarChoices(s, server(), async () => false)
  forgetOldAvatarChoice(s, 'avatar_shape') // picked Theme by hand
  await carryOldAvatarChoices(s, server(), async (patch) => { sent.push(patch); return true })
  assert.deepEqual(sent, [{ presence_style: 'ring' }])
})

test('nothing to send still clears the old copy, without a save', async () => {
  const s = storage(old)
  await carryOldAvatarChoices(s, server('soft', 'off'), async () => assert.fail('no save'))
  assert.deepEqual(s.layout(), layout())
})
