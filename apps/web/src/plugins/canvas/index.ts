import { type CommandContext, registerPlugin } from '..'
import { api } from '../../lib/api'
import { store } from '../../lib/store.svelte'
import { router } from '../../lib/router.svelte'
import { openObject } from '../../lib/objects.svelte'
import type { LiveObject } from '../../lib/types'
import Card from './Card.svelte'
import View from './View.svelte'
import Tile from './Tile.svelte'

const channel = () => router.route.name === 'channel' ? store.channel(router.route.id) : undefined
const enabled = () => store.settings.canvas_enabled && channel()?.kind !== 'voice' && !!channel()
async function create(channelId: string, name = '') {
  const object = await api.post<LiveObject>(`/channels/${channelId}/objects`, { kind: 'canvas', name: name || 'Untitled canvas' })
  await store.loadLatest(channelId)
  return object
}
registerPlugin({
  objectKinds: { canvas: { card: Card, view: View, tile: Tile } },
  get slashCommands() { return enabled() ? [{ name: 'canvas', hint: 'Start a shared drawing board', run: async (ctx: CommandContext) => { await create(ctx.channelId, ctx.args) } }] : [] },
  get paletteActions() { const c = channel(); return enabled() && c ? [{ id: 'canvas-new', label: `New canvas in #${store.title(c)}`, hint: 'canvas', run: async () => { try { openObject(await create(c.id)) } catch (err) { alert((err as Error).message) } } }] : [] },
})
