// The server decides what deserves an alert (mentions, DMs, followed rooms) and sends a
// `notification` event. We only decide whether you're already looking at it.
import { store } from './store.svelte'
import { router } from './router.svelte'

let handled = 0

export const notify = {
  attach() {
    $effect(() => {
      const fresh = store.alerts.slice(handled)
      handled = store.alerts.length
      for (const a of fresh) {
        const c = store.channel(a.message.channel_id)
        if (!c) continue
        const looking = document.visibilityState === 'visible' && router.route.name === 'channel' && router.route.id === c.id
        if (!looking) fire(a.message.author_id, c.kind === 'dm' ? '' : ` in #${c.name}`, a.message.content, c.id)
      }
    })
  },
  async ask() {
    if (!('Notification' in window)) return false
    return (await Notification.requestPermission()) === 'granted'
  },
}

function fire(authorId: string, where: string, body: string, channelId: string) {
  if (!('Notification' in window) || Notification.permission !== 'granted') return
  const n = new Notification(`${store.settings.instance_name}: ${store.name(authorId)}${where}`, { body: body.slice(0, 140) || 'sent a file', tag: channelId, silent: !store.layout.sounds })
  n.onclick = () => { window.focus(); router.go(`/c/${channelId}`); n.close() }
}
