import { test } from 'node:test'
import assert from 'node:assert/strict'
import { pickMachine } from '../apps/web/src/plugins/terminal/machine.ts'

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
  // Two machines with one name are offered once.
  assert.equal(error([pi, { ...nas, name: 'pi' }], 'x'), 'No machine called x. Try /terminal pi.')
})
