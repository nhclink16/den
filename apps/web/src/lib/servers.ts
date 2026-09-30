// Game servers as the Servers page shows them. Everything a relay can vary
// (buttons, numbers) is rendered from what it declared, never assumed.
import type { GameServer, ServerAction, ServerState, ServerStat, StatUnit } from './types'

const GAMES: Record<string, { label: string; glyph: string }> = {
  minecraft: { label: 'Minecraft', glyph: '⛏' },
}
/** The sidebar name: the game, not its MOTD. An unknown game falls back to the server's name. */
export const gameLabel = (s: Pick<GameServer, 'game' | 'name'>) => GAMES[s.game]?.label ?? s.name
export const gameGlyph = (game: string) => GAMES[game]?.glyph ?? '🎮'

export const stateLabel = (s: Pick<GameServer, 'state' | 'connected'>): string => {
  if (!s.connected) return 'Unreachable'
  return { up: 'Up', starting: 'Starting', stopping: 'Stopping', asleep: 'Asleep', down: 'Down' }[s.state]
}

/** The dot: lit when up, dim asleep, red down or unreachable, amber in between. */
export const dotTone = (s: Pick<GameServer, 'state' | 'connected'>): 'up' | 'asleep' | 'down' | 'busy' =>
  !s.connected || s.state === 'down' ? 'down' : s.state === 'up' ? 'up' : s.state === 'asleep' ? 'asleep' : 'busy'

/** Buttons this person can press right now. Den enforces the same rules. */
export function usableActions(s: Pick<GameServer, 'actions' | 'state' | 'connected'>, admin: boolean): ServerAction[] {
  if (!s.connected) return []
  return s.actions.filter((a) => (admin || !a.admin_only) && a.states.includes(s.state as ServerState))
}

export function duration(seconds: number): string {
  const s = Math.max(0, Math.round(seconds))
  if (s < 60) return `${s}s`
  const m = Math.floor(s / 60), h = Math.floor(m / 60), d = Math.floor(h / 24)
  if (d) return `${d}d ${h % 24}h`
  if (h) return `${h}h ${m % 60}m`
  return `${m}m`
}

/** Hours for playtime, where minutes stop mattering after the first few hours. */
export function playtime(seconds: number): string {
  if (seconds < 3600) return seconds < 60 ? (seconds > 0 ? '<1m' : '—') : `${Math.floor(seconds / 60)}m`
  const h = seconds / 3600
  return `${h < 10 ? h.toFixed(1) : Math.round(h)}h`
}

export function when(unix: number, now = Date.now()): string {
  const d = new Date(unix * 1000), today = new Date(now)
  const time = d.toLocaleTimeString([], { hour: 'numeric', minute: '2-digit' })
  const days = Math.round((new Date(today.getFullYear(), today.getMonth(), today.getDate()).getTime() - new Date(d.getFullYear(), d.getMonth(), d.getDate()).getTime()) / 86_400_000)
  if (days <= 0) return time
  if (days === 1) return `yesterday ${time}`
  return d.toLocaleDateString([], { month: 'short', day: 'numeric' })
}

export function formatValue(value: number, unit: StatUnit, now = Date.now()): string {
  switch (unit) {
    case 'tps': return value.toFixed(1)
    case 'percent': return `${Math.round(value)}%`
    case 'bytes': return value >= 1024 ** 3 ? `${(value / 1024 ** 3).toFixed(1)} GB` : `${Math.round(value / 1024 ** 2)} MB`
    case 'seconds': return duration(value)
    case 'timestamp': return when(value, now)
    default: return Number.isInteger(value) ? String(value) : value.toFixed(1)
  }
}
export const formatStat = (s: ServerStat, now = Date.now()) => formatValue(s.value, s.unit, now)

/** Below 18 ticks a second players notice; say so in words, not only a number. */
export const lagging = (s: ServerStat) => s.unit === 'tps' && s.value < 18
