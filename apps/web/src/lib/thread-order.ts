import type { ThreadSummary } from './types'

// Strip order, kept out of the Store so the rule can be exercised directly.
//
// The order is decided when a room is entered and then held for that visit:
// activity descending, ties by ID. Anything discovered afterwards is APPENDED
// rather than sorted into place, because a conversation that jumps under the
// pointer as you reach for it is worse than one listed slightly out of order.

/// Add ids that are not already known, preserving the existing order.
export function appendNew(seen: string[], incoming: string[]): string[] {
  const add = incoming.filter((id) => !seen.includes(id))
  return add.length ? [...seen, ...add] : seen
}

/// Recompute the whole order. Called on room entry, never on an update.
export function byActivity(ids: string[], meta: (id: string) => ThreadSummary | undefined): string[] {
  return [...ids].sort((a, b) => {
    const x = meta(a), y = meta(b)
    if (!x || !y) return 0
    return y.last_activity_at.localeCompare(x.last_activity_at) || y.id.localeCompare(x.id)
  })
}
