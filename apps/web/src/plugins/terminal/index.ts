import { registerPlugin, type CommandContext } from '..'
import { api } from '../../lib/api'
import { store } from '../../lib/store.svelte'
import { router } from '../../lib/router.svelte'
import { openObject } from '../../lib/objects.svelte'
import type { LiveObject } from '../../lib/types'
import { catalog, terminals } from './state.svelte'
import Card from './Card.svelte'
import View from './View.svelte'
import Tile from './Tile.svelte'
import RequestCard from './RequestCard.svelte'
async function create(machine: string, channelId?: string) {
  await catalog()
  const matches = terminals.hosts.filter(h => h.online && (h.name === machine || h.id === machine))
  if (matches.length !== 1) throw new Error('Choose a machine from Settings, Machines.')
  const o = await api.post<LiveObject>(`/hosts/${matches[0].id}/sessions`, { channel_id: channelId })
  await store.resync(); await store.loadLatest(o.channel_id); openObject(o); return o
}
registerPlugin({
  objectKinds: { terminal: { card: Card, view: View, tile: Tile }, access_request: { card: RequestCard } },
  slashCommands: [{ name: 'terminal', hint: 'Open a terminal on a machine', run: async (ctx: CommandContext) => {await create(ctx.args.trim(), ctx.channelId)} }],
  get paletteActions() { return terminals.hosts.map(h => ({ id: `terminal-${h.id}`, label: `Terminal on ${h.name}`, hint: h.online ? 'terminal' : 'offline', disabled: !h.online, run: async () => { const channel = router.route.name === 'channel' ? router.route.id : undefined; try { await create(h.id, channel) } catch (e) { store.toast = (e as Error).message } } })) },
})
