import { type CommandContext, registerPlugin } from '..'
import { api } from '../../lib/api'
import { store } from '../../lib/store.svelte'
import { router } from '../../lib/router.svelte'
import { openObject } from '../../lib/objects.svelte'
import type { Conversation } from '../../lib/conversation'
import type { LiveObject } from '../../lib/types'
import Card from './Card.svelte'
import View from './View.svelte'
import Tile from './Tile.svelte'

const channel = () => router.route.name === 'channel' ? store.channel(router.route.id) : undefined
const enabled = () => store.settings.canvas_enabled && channel()?.kind !== 'voice' && !!channel()
/// `into` places the card in the conversation the command was typed in. The
/// server takes the same context as a message, so a canvas started in a thread
/// lands there instead of the room.
async function create(channelId: string, name = '', into?: Conversation, quote?: string | null) {
  // Captured before the request: the follow-up refresh must not be decided by
  // whichever account is active when the response finally lands.
  const owner = store.drafts
  const token = owner.token
  const mine = () => store.drafts === owner && owner.holds(token)
  const thread = into?.rootId ? store.threadForRoot(into.rootId)?.id : undefined
  const object = await api.post<LiveObject>(`/channels/${channelId}/objects`, {
    kind: 'canvas',
    name: name || 'Untitled canvas',
    thread_id: thread ?? null,
    // The quote the composer actually held wins; the root is only the fallback
    // that PLACES a first card in a conversation with no thread yet.
    reply_to: quote ?? (thread ? null : into?.rootId ?? null),
  })
  if (!mine()) return object
  if (into?.rootId) { if (thread) await store.loadThreadMessages(thread) } else await store.loadLatest(channelId)
  return object
}
registerPlugin({
  objectKinds: { canvas: { card: Card, view: View, tile: Tile } },
  get slashCommands() { return enabled() ? [{ name: 'canvas', hint: 'Start a shared drawing board', run: async (ctx: CommandContext) => { await create(ctx.channelId, ctx.args, ctx.conversation, ctx.replyToId) } }] : [] },
  get paletteActions() { const c = channel(); return enabled() && c ? [{ id: 'canvas-new', label: `New canvas in #${store.title(c)}`, hint: 'canvas', run: async () => {
        // create() returns the object even when it refused the follow-up because
        // the account turned over, so the dock is guarded here too: a completed
        // old-account canvas must not open in whoever is signed in now.
        const owner = store.drafts, token = owner.token
        try {
          const made = await create(c.id)
          if (store.drafts === owner && owner.holds(token)) openObject(made)
        } catch (err) { alert((err as Error).message) }
      } }] : [] },
})
