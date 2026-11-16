import { test } from 'node:test'
import assert from 'node:assert/strict'
import { pickMachine } from '../apps/web/src/plugins/terminal/machine.ts'
import { edited } from '../apps/web/src/plugins/canvas/edited.ts'

const pi = { id: 'h1', name: 'pi', online: true }
const nas = { id: 'h2', name: 'nas', online: true }
const cabin = { id: 'h3', name: 'cabin-pi', online: false }

test('/terminal picks the one online machine by name or ID', () => {
  assert.deepEqual(pickMachine([pi, nas], 'pi'), { id: 'h1' })
  assert.deepEqual(pickMachine([pi, nas], 'h2'), { id: 'h2' })
})

test('/terminal says why no machine was picked', () => {
  const error = (hosts: typeof pi[], typed: string) => (pickMachine(hosts, typed) as { error: string }).error
  assert.equal(error([], 'cabin-pi'), 'No machine called cabin-pi is online. Machines are set up in Settings › Machines.')
  assert.equal(error([cabin], 'cabin-pi'), 'No machine called cabin-pi is online. Machines are set up in Settings › Machines.')
  assert.equal(error([], ''), 'No machine is online. Machines are set up in Settings › Machines.')
  assert.equal(error([pi, nas], 'cabin-pi'), 'No machine called cabin-pi. Try /terminal pi or /terminal nas.')
  assert.equal(error([pi, nas, cabin], 'cabin-pi'), 'cabin-pi isn’t online. Try /terminal pi or /terminal nas.')
  assert.equal(error([pi, nas], ''), 'Which machine? Try /terminal pi or /terminal nas.')
  assert.equal(error([pi, { ...nas, name: 'pi' }], 'pi'), 'Two machines are called pi. Use the machine’s ID.')
  assert.equal(error([pi, nas, { id: 'h4', name: 'desk', online: true }], 'x'), 'No machine called x. Try /terminal pi, /terminal nas or /terminal desk.')
  // Two machines with one name are offered once.
  assert.equal(error([pi, { ...nas, name: 'pi' }], 'x'), 'No machine called x. Try /terminal pi.')
})

test('a canvas card says when it last changed', () => {
  const now = new Date(2026, 10, 15, 15, 0)
  const ago = (ms: number) => new Date(now.getTime() - ms).toISOString()
  assert.equal(edited(ago(20_000), now), 'just now')
  assert.equal(edited(ago(5 * 60_000), now), '5 min ago')
  assert.equal(edited(ago(59 * 60_000 + 59_000), now), '59 min ago')
  assert.equal(edited(ago(60 * 60_000), now), '1 hour ago')
  assert.equal(edited(ago(119 * 60_000), now), '1 hour ago')
  assert.equal(edited(ago(5 * 3_600_000), now), '5 hours ago')
  assert.equal(edited(new Date(2026, 10, 14, 9, 0).toISOString(), now), 'yesterday')
  assert.equal(edited(new Date(2026, 10, 11, 9, 0).toISOString(), now), new Date(2026, 10, 11).toLocaleDateString([], { weekday: 'long' }))
  // A clock a little behind the server's never shows a negative age.
  assert.equal(edited(ago(-30_000), now), 'just now')
})
