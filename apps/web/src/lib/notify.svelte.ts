// The server decides what deserves an alert (mentions, DMs, followed rooms) and sends a
// `notification` event. We only decide whether you're already looking at it.
import { native } from './native'
import { sounds } from './sounds'
import type { Event } from './types'
import type { Store } from './store.svelte'
import { store } from './store.svelte'
import { router } from './router.svelte'

export function lookingAt(source: Store, channelId: string) {
  return source.active && document.hasFocus() && document.visibilityState === 'visible' && router.route.name === 'channel' && router.route.id === channelId
}
export function receiveAlert(source: Store, alert: Extract<Event, { type: 'notification' }>) {
  const c = source.channel(alert.message.channel_id)
  if (!c || lookingAt(source, c.id)) return
  void sounds.play(alert.reason === 'subscribed_channel' ? 'message' : alert.reason, source)
  if (!native) fire(alert.message.author_id, c.kind === 'dm' ? '' : ` in #${c.name}`, alert.message.content, c.id)
}
export const notify = {
  attach() {},
  async ask() {
    if (native) return true
    if (!('Notification' in window)) return false
    return (await Notification.requestPermission()) === 'granted'
  },
}

function fire(authorId: string, where: string, body: string, channelId: string) {
  if (!('Notification' in window) || Notification.permission !== 'granted') return
  const n = new Notification(`${store.settings.instance_name}: ${store.name(authorId)}${where}`, { body: body.slice(0, 140) || 'sent a file', tag: channelId, silent: true })
  n.onclick = () => { window.focus(); router.go(`/c/${channelId}`); n.close() }
}
