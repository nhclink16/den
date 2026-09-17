import { registerPlugin, type CommandContext } from '..'
import { api } from '../../lib/api'
import { store } from '../../lib/store.svelte'
import { router } from '../../lib/router.svelte'
import { openObject } from '../../lib/objects.svelte'
import type { Conversation } from '../../lib/conversation'
import type { LiveObject } from '../../lib/types'
import { catalog, terminals } from './state.svelte'
import Card from './Card.svelte'
import View from './View.svelte'
import Tile from './Tile.svelte'
import RequestCard from './RequestCard.svelte'
/// `into` places the session card in the conversation it was opened from, using
/// the same context fields the server already takes. Opening a terminal from a
/// thread panel puts the card in that thread, not the room.
async function create(machine: string, channelId?: string, into?: Conversation, quote?: string | null) {
  // Everything this command needs is captured BEFORE the catalog request. The
  // globals it used to read afterwards follow the active instance, so an account
  // or server switch while the catalog was in flight would have opened a machine
  // under whoever is active by then.
  const owner = store.drafts
  const token = owner.token
  const placement = into?.rootId ? store.threadForRoot(into.rootId)?.id : undefined
  const rootId = into?.rootId ?? null
  const post = api.post
  const mine = () => store.drafts === owner && owner.holds(token)
  await catalog()
  if (!mine()) return undefined
  const matches = terminals.hosts.filter(h => h.online && (h.name === machine || h.id === machine))
  if (matches.length !== 1) throw new Error('Choose a machine from Settings, Machines.')
  const thread = placement ?? (rootId ? store.threadForRoot(rootId)?.id : undefined)
  const o = await post<LiveObject>(`/hosts/${matches[0].id}/sessions`, {
    channel_id: channelId,
    thread_id: thread ?? null,
    reply_to: quote ?? (thread ? null : rootId),
  })
  // A completed old-account action must not dock its object in the new account.
  if (!mine()) return undefined
  await store.resync()
  if (!mine()) return undefined
  if (thread) await store.loadThreadMessages(thread); else await store.loadLatest(o.channel_id)
  if (mine()) openObject(o)
  return o
}
registerPlugin({
  objectKinds: { terminal: { card: Card, view: View, tile: Tile }, access_request: { card: RequestCard } },
  slashCommands: [{ name: 'terminal', hint: 'Open a terminal on a machine', run: async (ctx: CommandContext) => {await create(ctx.args.trim(), ctx.channelId, ctx.conversation, ctx.replyToId)} }],
  get paletteActions() { return terminals.hosts.map(h => ({ id: `terminal-${h.id}`, label: `Terminal on ${h.name}`, hint: h.online ? 'terminal' : 'offline', disabled: !h.online, run: async () => { const channel = router.route.name === 'channel' ? router.route.id : undefined; try { await create(h.id, channel) } catch (e) { store.toast = (e as Error).message } } })) },
})
