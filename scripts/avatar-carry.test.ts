import { test } from 'node:test'
import assert from 'node:assert/strict'
import { oldAvatarChoices, takeOldAvatarChoices } from '../apps/web/src/lib/avatar-carry.ts'

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

test('the old copy is taken before the save, so a second sync or Store sends nothing', () => {
  const s = storage(layout({ avatarShape: 'round', presence: 'ring' }))
  assert.deepEqual(takeOldAvatarChoices(s, 'den.layout', server())?.patch, { avatar_shape: 'round', presence_style: 'ring' })
  assert.deepEqual(s.layout(), layout())
  assert.equal(takeOldAvatarChoices(s, 'den.layout', server()), null)
})

test('a failed save puts the old copy back for the next sync, keeping later layout changes', () => {
  const s = storage(layout({ avatarShape: 'round', presence: 'ring' }))
  const taken = takeOldAvatarChoices(s, 'den.layout', server())!
  s.setItem('den.layout', JSON.stringify(layout({ sidebar: false })))
  taken.restore()
  assert.deepEqual(s.layout(), layout({ sidebar: false, avatarShape: 'round', presence: 'ring' }))
  assert.deepEqual(takeOldAvatarChoices(s, 'den.layout', server())?.patch, { avatar_shape: 'round', presence_style: 'ring' })
})
