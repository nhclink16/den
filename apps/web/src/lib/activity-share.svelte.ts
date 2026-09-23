// The desktop app reports the focused game or app; this decides whether to share
// it and keeps it alive on every signed-in server. Settings are per device, and
// sharing starts on: the app name is all that ever leaves the machine.
import { native, invoke, listen } from './native'
import { instances } from './store.svelte'
import type { ActivityKind } from './types'

type Detected = { kind: ActivityKind; name: string }
type Prefs = { share: boolean; hidden: string[]; seen: string[] }
const KEY = 'den.activity'
const REFRESH = 60_000, TTL = 90

function load(): Prefs {
  const fallback: Prefs = { share: true, hidden: [], seen: [] }
  try { return { ...fallback, ...JSON.parse(localStorage.getItem(KEY) || '{}') } } catch { return fallback }
}

class ActivityShare {
  prefs = $state<Prefs>(load())
  detected = $state<Detected | null>(null)
  /** What is shared right now, after this device's choices. */
  get shared(): Detected | null {
    const d = this.detected
    return d && this.prefs.share && !this.prefs.hidden.includes(d.name) ? d : null
  }
  private sentTo = new Set<string>()
  private attached = false

  attach() {
    if (!native || this.attached) return
    this.attached = true
    const receive = (d: Detected | null) => {
      this.detected = d
      if (d && !this.prefs.seen.includes(d.name)) this.save({ seen: [d.name, ...this.prefs.seen].slice(0, 20) })
      void this.push()
    }
    void listen<Detected | null>('activity', receive)
    void invoke<Detected | null>('activity_current').then(receive).catch(() => {})
    setInterval(() => void this.push(), REFRESH)
  }

  save(patch: Partial<Prefs>) {
    this.prefs = { ...this.prefs, ...patch }
    localStorage.setItem(KEY, JSON.stringify(this.prefs))
    void this.push()
  }
  hide(name: string, hidden: boolean) {
    this.save({ hidden: hidden ? [...new Set([...this.prefs.hidden, name])] : this.prefs.hidden.filter((n) => n !== name) })
  }

  private async push() {
    const shared = this.shared
    await Promise.all(instances.all.map(async (s) => {
      try {
        if (shared) {
          await s.api.put('/users/me/activities/desktop', { kind: shared.kind, name: shared.name, ttl_seconds: TTL })
          this.sentTo.add(s.origin)
        } else if (this.sentTo.has(s.origin)) {
          await s.api.del('/users/me/activities/desktop')
          this.sentTo.delete(s.origin)
        }
      } catch { /* offline or an older server; the next refresh tries again */ }
    }))
  }
}
export const activityShare = new ActivityShare()
