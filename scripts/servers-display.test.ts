import { test } from 'node:test'
import assert from 'node:assert/strict'
import { usableActions, dotTone, formatValue, playtime, stateLabel } from '../apps/web/src/lib/servers.ts'
import type { GameServer, ServerAction } from '../apps/web/src/lib/types.ts'

const action = (id: string, admin_only: boolean, states: ServerAction['states']): ServerAction => ({ id, label: id, admin_only, states })
const server = (extra: Partial<GameServer> = {}): GameServer => ({
  slug: 'minecraft', game: 'minecraft', name: 'Den', details: [], address: null, icon_url: null,
  connected: true, state: 'up', players: [], max_players: 5, stats: [], updated_at: 0,
  actions: [action('save', false, ['up']), action('start', false, ['down', 'asleep']), action('restart', true, ['up', 'starting']), action('stop', true, ['up', 'starting'])],
  ...extra,
})

test('the relay’s declared buttons decide what shows, by state and role', () => {
  assert.deepEqual(usableActions(server(), false).map((a) => a.id), ['save'])
  assert.deepEqual(usableActions(server(), true).map((a) => a.id), ['save', 'restart', 'stop'])
  assert.deepEqual(usableActions(server({ state: 'asleep' }), false).map((a) => a.id), ['start'])
  assert.deepEqual(usableActions(server({ connected: false, state: 'down' }), true), [], 'nothing runs without the relay')
})

test('an unreachable relay reads as down, never as its last state', () => {
  assert.equal(stateLabel(server({ connected: false })), 'Unreachable')
  assert.equal(dotTone(server({ connected: false })), 'down')
  assert.equal(dotTone(server({ state: 'asleep' })), 'asleep')
  assert.equal(dotTone(server({ state: 'starting' })), 'busy')
})

test('numbers are formatted by the unit the relay declared', () => {
  assert.equal(formatValue(19.96, 'tps'), '20.0')
  assert.equal(formatValue(2.25 * 1024 ** 3, 'bytes'), '2.3 GB')
  assert.equal(formatValue(11_520, 'seconds'), '3h 12m')
  assert.equal(formatValue(13.6, 'percent'), '14%')
  assert.equal(playtime(78 * 3600), '78h')
  assert.equal(playtime(8.4 * 3600), '8.4h')
  assert.equal(playtime(0), '—')
})
