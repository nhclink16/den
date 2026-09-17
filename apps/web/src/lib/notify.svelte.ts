// The server decides what deserves an alert (mentions, DMs, followed rooms) and sends a
// `notification` event. We only decide whether you're already looking at it.
import { native } from './native'
import { sounds } from './sounds'
import type { Event } from './types'
import type { Store } from './store.svelte'
import { instances } from './store.svelte'
import { router } from './router.svelte'

/// What the chat view is actually showing, published by ChannelView because only
/// it knows whether the room is beside the panel or behind it. A room with a
/// conversation on top of it in a narrow window is not being read.
export const viewing = $state({ channelId: '', threadId: undefined as string | undefined, roomVisible: false })

/// Suppression is per CONVERSATION, not per room. In a wide window both the room
/// and one conversation can qualify; a collapsed or hidden one never does.
export function lookingAt(source: Store, channelId: string, threadId?: string | null) {
  if (!source.active || !document.hasFocus() || document.visibilityState !== 'visible') return false
  if (router.route.name !== 'channel' || router.route.id !== channelId) return false
  if (viewing.channelId !== channelId) return false
  return threadId ? viewing.threadId === threadId : viewing.roomVisible
}

export function receiveAlert(source: Store, alert: Extract<Event, { type: 'notification' }>) {
  const c = source.channel(alert.message.channel_id)
  if (!c || lookingAt(source, c.id, alert.message.thread_id)) return
  void sounds.play(alert.reason === 'subscribed_channel' ? 'message' : alert.reason, source)
  if (!native) {
    fire(source, alert.message.author_id, c.kind === 'dm' ? '' : ` in #${c.name}`,
      alert.message.content, c.id, alert.message.id)
  }
}

export const notify = {
  attach() {},
  async ask() {
    if (native) return true
    if (!('Notification' in window)) return false
    return (await Notification.requestPermission()) === 'granted'
  },
}

function fire(source: Store, authorId: string, where: string, body: string, channelId: string, messageId: string) {
  if (!('Notification' in window) || Notification.permission !== 'granted') return
  // The closure captures THIS account and THIS message. Reading the active proxy
  // when the click finally happens would open whichever server is selected then.
  const n = new Notification(`${source.settings.instance_name}: ${source.name(authorId)}${where}`,
    { body: body.slice(0, 140) || 'sent a file', tag: channelId, silent: true })
  n.onclick = () => {
    window.focus()
    void goToMessage(source, channelId, messageId)
    n.close()
  }
}

/// The one destination resolver every entry point uses: search results, quoted
/// references, root badges and notification taps all land on the message itself,
/// inside whatever conversation actually holds it.
export async function goToMessage(source: Store, channelId: string, messageId?: string) {
  // The instance that owns the message is the one that must be showing when we
  // navigate; the lookup happens on it either way, but routing without selecting
  // it would point the active view at another server's message.
  const lifetime = source.drafts
  const token = lifetime.token
  const alive = () => lifetime.holds(token) && instances.all.includes(source)
  if (!alive()) return
  // Only when the instance actually changes: selecting the active one clears the
  // object dock and pushes '/', which is not something navigating should do.
  if (instances.active !== source) instances.select(source)
  if (!messageId) { router.go(`/c/${channelId}`); return }
  const found = await source.locate(messageId, channelId).catch(() => undefined)
  // A click that resolved after the account went away must not navigate at all.
  if (!alive()) return
  if (instances.active !== source) instances.select(source)
  if (!found) { router.go(`/c/${channelId}?m=${messageId}`); return }
  router.go(found.threadId
    ? `/c/${found.channelId}/t/${found.threadId}?m=${messageId}`
    : `/c/${found.channelId}?m=${messageId}`)
}
