import { dayLabel } from '../../lib/time.ts'

/// When a canvas last changed, the way the card says it: "just now", "5 min ago", "3 hours ago",
/// then the day ("yesterday", "Tuesday", "Nov 3").
export function edited(iso: string, now = new Date()): string {
  const mins = Math.floor((now.getTime() - new Date(iso).getTime()) / 60_000)
  if (mins < 1) return 'just now'
  if (mins < 60) return `${mins} min ago`
  if (mins < 24 * 60) return mins < 120 ? '1 hour ago' : `${Math.floor(mins / 60)} hours ago`
  const day = dayLabel(iso, now)
  return day === 'Yesterday' ? 'yesterday' : day
}
