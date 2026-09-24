// How activities read on a card or in the people list. Pure, so it is tested directly.
import type { Activity, ActivityKind } from './types'
// Games first, background listening after, the app someone merely has open last.
const RANK: Record<ActivityKind, number> = { playing: 0, watching: 1, listening: 2, working: 3, using: 4 }

/** Someone's activities to show: one per thing, most telling first. */
export function shownActivities(list: Activity[] | undefined): Activity[] {
  const byThing = new Map<string, Activity>()
  for (const a of list ?? []) {
    // The desktop and the Minecraft relay can both say "Playing Minecraft"; keep
    // the one that says more.
    const key = `${a.kind}:${a.name.toLowerCase()}`
    const had = byThing.get(key)
    if (!had || (!had.details && a.details) || (!had.image_url && a.image_url)) byThing.set(key, a)
  }
  return [...byThing.values()].sort((a, b) => RANK[a.kind] - RANK[b.kind] || b.started_at - a.started_at)
}

export function activityVerb(kind: ActivityKind): string {
  return { playing: 'Playing', listening: 'Listening to', watching: 'Watching', using: 'Using', working: 'Working on' }[kind]
}

/** "for 12m", "for 2h 5m", or "just started". */
export function activitySince(startedAt: number, now = Date.now()): string {
  const minutes = Math.floor((now - startedAt) / 60_000)
  if (minutes < 1) return 'just started'
  if (minutes < 60) return `for ${minutes}m`
  const hours = Math.floor(minutes / 60), rest = minutes % 60
  return `for ${hours}h${rest ? ` ${rest}m` : ''}`
}

/** The glyph beside each kind of activity. */
export const activityIcon = { playing: 'game', listening: 'music', watching: 'screen', using: 'window', working: 'bot' } as const

/** A device's choices about what the desktop app may share. */
export type SharePrefs = { share: boolean; hidden: string[]; allowed: string[]; askApps: boolean }
/** Games share on their own; any other app only once its owner says yes to it. */
export function shareDecision(p: SharePrefs, d: { kind: ActivityKind; name: string } | null): 'share' | 'keep' | 'ask' {
  if (!d || !p.share || p.hidden.includes(d.name)) return 'keep'
  if (d.kind === 'playing' || p.allowed.includes(d.name)) return 'share'
  return p.askApps ? 'ask' : 'keep'
}
