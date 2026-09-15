import { call } from './call.svelte'
import { instances } from './store.svelte'
import { native, invoke, listen } from './native'
import type { DesktopEvent } from './desktop'
import { router } from './router.svelte'
import type { Event } from './types'

class Desktop {
  platform = $state('')
  global = $state(false)
  updateReady = $state(false)
  private shortcut = ''
  private chain = Promise.resolve()
  attach() {
    if (!native) return
    const cleanup: (() => void)[] = []
    let stopped = false
    const on = async <T>(event: DesktopEvent, handler: (data: T) => void) => { const off = await listen<T>(event, handler); if (stopped) off(); else cleanup.push(off) }
    void invoke<string>('platform').then(p => this.platform = p)
    void on<boolean>('ptt', held => call.setHeld(held))
    void on<string>('tray-action', action => { if (action === 'mute') void call.toggleMic(); if (action === 'deafen') void call.toggleOutput() })
    void on<{ origin: string; channel: string }>('notification-open', ({ origin, channel }) => {
      const s = instances.all.find(s => s.origin === origin)
      if (s) { instances.select(s); router.go(`/c/${channel}`) }
    })
    const deep = (links: string[]) => { for (const raw of links) { try { const u = new URL(raw); if (u.protocol === 'den:' && u.hostname === 'join') instances.add(u.searchParams.get('url') || '', u.searchParams.get('invite') || '') } catch { /* unrelated URL */ } } }
    void on<string[]>('deep-link://new-url', deep)
    void invoke<string[]>('deep_links').then(deep)
    const check = () => void invoke<boolean>('update_check').then(ready => this.updateReady = ready).catch(() => {})
    check(); const timer = setInterval(check, 6 * 60 * 60 * 1000)
    const alert = (e: globalThis.Event) => {
      const { origin, alert: a } = (e as CustomEvent<{ origin: string; alert: Extract<Event, { type: 'notification' }> }>).detail
      const s = instances.all.find(s => s.origin === origin), c = s?.channel(a.message.channel_id)
      if (!s || !c || (s === instances.active && document.hasFocus() && document.visibilityState === 'visible' && router.route.name === 'channel' && router.route.id === c.id)) return
      const title = `${instances.all.length > 1 ? `${s.settings.instance_name}: ` : ''}${s.name(a.message.author_id)}${c.kind === 'dm' ? '' : ` in #${c.name}`}`
      void invoke('notify', { title, body: a.message.content.slice(0, 140) || 'Sent a file', origin, channel: c.id }).catch(() => {})
    }
    window.addEventListener('den-alert', alert)
    return () => { stopped = true; cleanup.forEach(off => off()); clearInterval(timer); window.removeEventListener('den-alert', alert); void invoke('ptt_register', { key: null }) }
  }
  syncShortcut(key: string) {
    if (!native || key === this.shortcut) return
    this.shortcut = key
    this.chain = this.chain.catch(() => {}).then(async () => {
      this.global = false; call.setHeld(false)
      try { await invoke('tray_state', { inCall: !!call.room, muted: !call.micOn, deafened: call.outputMuted }); await invoke('ptt_register', { key: key || null }); this.global = !!key } catch (e) { call.error = `Global push-to-talk: ${e}` }
    })
  }
  async restart() { try { await invoke('update_restart') } catch (e) { instances.active.toast = `Update failed: ${e}` } }
}
export const desktop = new Desktop()
