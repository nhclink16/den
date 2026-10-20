import { test } from 'node:test'
import assert from 'node:assert/strict'
import { oldAvatarChoices } from '../apps/web/src/lib/avatar-carry.ts'

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
