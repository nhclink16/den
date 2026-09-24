// The desktop app reports the focused game or app; this decides whether to share
// it and keeps it alive on every signed-in server. Settings are per device. Games
// share on their own; any other app is asked about once, since "Using Firefox" is
// more personal than "Playing Minecraft". The app name is all that leaves the machine.
import { native, invoke, listen } from './native'
import { instances, store } from './store.svelte'
import { shareDecision, type SharePrefs } from './activity'
import type { ActivityKind } from './types'

type Detected = { kind: ActivityKind; name: string }
type Prefs = SharePrefs & { seen: string[]; games: string[]; noticed: boolean }
const KEY = 'den.activity'
const REFRESH = 60_000, TTL = 90

function load(): Prefs {
  const fallback: Prefs = { share: true, hidden: [], allowed: [], askApps: true, seen: [], games: [], noticed: false }
  try { return { ...fallback, ...JSON.parse(localStorage.getItem(KEY) || '{}') } } catch { return fallback }
}

class ActivityShare {
  prefs = $state<Prefs>(load())
  detected = $state<Detected | null>(null)
  /** What is shared right now, after this device's choices. */
  get shared(): Detected | null {
    return shareDecision(this.prefs, this.detected) === 'share' ? this.detected : null
  }
  /** An app waiting on "Share that you're using it?", until answered or it loses focus. */
  get asking(): Detected | null {
    return shareDecision(this.prefs, this.detected) === 'ask' ? this.detected : null
  }
  private sentTo = new Set<string>()
  private attached = false

  attach() {
    if (!native || this.attached) return
    this.attached = true
    const receive = (d: Detected | null) => {
      this.detected = d
      // Games share without asking, so say so once, the first time one does.
      if (d && shareDecision(this.prefs, d) === 'share' && !this.prefs.noticed) {
        store.toast = `People can see you're playing ${d.name}. Turn it off or hide games in Settings › Profile › Activity.`
        this.save({ noticed: true })
      }
      if (d && !this.prefs.seen.includes(d.name)) this.save({ seen: [d.name, ...this.prefs.seen].slice(0, 20) })
      if (d?.kind === 'playing' && !this.prefs.games.includes(d.name)) this.save({ games: [d.name, ...this.prefs.games].slice(0, 20) })
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
  /** Show or hide one game or app. Showing an app also answers its question. */
  hide(name: string, hidden: boolean) {
    const without = (list: string[]) => list.filter((n) => n !== name)
    this.save(hidden
      ? { hidden: [...without(this.prefs.hidden), name], allowed: without(this.prefs.allowed) }
      : { hidden: without(this.prefs.hidden), allowed: [...without(this.prefs.allowed), name] })
  }
  /** The answer to "Share that you're using this?": this app, not this app, or stop asking. */
  answer(choice: 'yes' | 'no' | 'never') {
    const d = this.asking
    if (!d) return
    if (choice === 'never') this.save({ askApps: false })
    else this.hide(d.name, choice === 'no')
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
