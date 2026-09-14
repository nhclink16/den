// Time the way people say it in a group chat, not the way a database does.
const DAY = 86_400_000

export function shortTime(iso: string): string {
  return new Date(iso).toLocaleTimeString([], { hour: 'numeric', minute: '2-digit' })
}

export function dayLabel(iso: string, now = new Date()): string {
  const d = new Date(iso)
  const start = new Date(now.getFullYear(), now.getMonth(), now.getDate()).getTime()
  const diff = start - new Date(d.getFullYear(), d.getMonth(), d.getDate()).getTime()
  if (diff <= 0) return 'Today'
  if (diff <= DAY) return 'Yesterday'
  if (diff < 6 * DAY) return d.toLocaleDateString([], { weekday: 'long' })
  return d.toLocaleDateString([], { month: 'short', day: 'numeric', year: d.getFullYear() === now.getFullYear() ? undefined : 'numeric' })
}

export function sameDay(a: string, b: string): boolean {
  const x = new Date(a), y = new Date(b)
  return x.getFullYear() === y.getFullYear() && x.getMonth() === y.getMonth() && x.getDate() === y.getDate()
}

export function within(a: string, b: string, ms: number): boolean {
  return Math.abs(new Date(a).getTime() - new Date(b).getTime()) < ms
}

export function bytes(n: number): string {
  if (n < 1024) return `${n} B`
  if (n < 1024 ** 2) return `${(n / 1024).toFixed(0)} KB`
  if (n < 1024 ** 3) return `${(n / 1024 ** 2).toFixed(1)} MB`
  return `${(n / 1024 ** 3).toFixed(2)} GB`
}
