// Quiet by default. A message notifies only if it mentions you, is a DM, or is in a channel
// you subscribed to, and only when the tab is hidden or another channel is focused.
import { store } from './store.svelte'
import { router } from './router.svelte'
import { mentions } from './markdown'
import type { Message } from './types'

let seen = new Set<string>()

function shouldNotify(m: Message): boolean {
  if (!store.me || m.author_id === store.me.id) return false
  const c = store.channel(m.channel_id)
  if (!c) return false
  const focused = document.visibilityState === 'visible' && router.route.name === 'channel' && router.route.id === c.id
  if (focused) return false
  if (c.kind === 'dm') return true
  if (store.prefs.subscribed.includes(c.id)) return true
  return mentions(m.content, store.users).includes(store.me.id)
}

export const notify = {
  attach() {
    // Poll the store for new messages: simpler than threading a callback through the socket.
    $effect(() => {
      for (const [, list] of store.messages) {
        const last = list.at(-1)
        if (!last || seen.has(last.id)) continue
        seen.add(last.id)
        if (seen.size > 2000) seen = new Set([...seen].slice(-500))
        if (shouldNotify(last)) fire(last)
      }
    })
  },
  async ask() {
    if (!('Notification' in window)) return false
    return (await Notification.requestPermission()) === 'granted'
  },
}

function fire(m: Message) {
  const c = store.channel(m.channel_id)!
  const who = store.name(m.author_id)
  const where = c.kind === 'dm' ? '' : ` in #${c.name}`
  if ('Notification' in window && Notification.permission === 'granted') {
    const n = new Notification(`${who}${where}`, { body: m.content.slice(0, 140) || 'sent a file', tag: m.channel_id, silent: !store.prefs.sounds })
    n.onclick = () => { window.focus(); router.go(`/c/${c.id}`); n.close() }
  }
}
