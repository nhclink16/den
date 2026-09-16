// The staging trays, keyed by conversation.
//
// Files are uploaded to a CHANNEL, but they are staged per conversation, so a
// file attached in the room cannot appear in a conversation inside that same
// room, or block its send. Every transform here targets exactly one key and
// returns a new record, which is what keeps one tray's progress, cancellation
// and post-send cleanup from reaching another.
//
// Kept separate from the reactive owner so the isolation rule can be tested
// without a browser.

/// What these transforms need of a staged file. PendingUpload satisfies it.
export type QueueItem = { id: number; done?: { id: string }; error?: string }
export type Queues<T extends QueueItem> = Record<string, T[]>

/// Replace one key's tray. An empty tray is removed rather than left behind, so
/// a conversation that was cleared stops being iterated.
export function put<T extends QueueItem>(queues: Queues<T>, key: string, items: T[]): Queues<T> {
  const next = { ...queues }
  if (items.length) next[key] = items
  else delete next[key]
  return next
}

export function tray<T extends QueueItem>(queues: Queues<T>, key: string): T[] {
  return queues[key] || []
}

export function append<T extends QueueItem>(queues: Queues<T>, key: string, item: T): Queues<T> {
  return put(queues, key, [...tray(queues, key), item])
}

/// Update one staged file in place. Progress and completion callbacks fire long
/// after the file was staged, so they address it by key and id, never by
/// position.
export function patch<T extends QueueItem>(queues: Queues<T>, key: string, id: number, part: Partial<T>): Queues<T> {
  return put(queues, key, tray(queues, key).map((p) => (p.id === id ? { ...p, ...part } : p)))
}

export function drop<T extends QueueItem>(queues: Queues<T>, key: string, id: number): Queues<T> {
  return put(queues, key, tray(queues, key).filter((p) => p.id !== id))
}

/// Consume exactly the uploads that were transmitted, from the tray they were
/// submitted from, even if a newer draft has since staged more there. Anything
/// not named stays, which is what keeps a slash command from eating attachments
/// it never sent.
export function consume<T extends QueueItem>(queues: Queues<T>, key: string, ids: string[]): Queues<T> {
  return put(queues, key, tray(queues, key).filter((p) => !p.done || !ids.includes(p.done.id)))
}

/// Still working: not finished and not failed. Gates that tray's send only.
export function busy<T extends QueueItem>(items: T[]): boolean {
  return items.some((p) => !p.done && !p.error)
}

/// Anything still uploading anywhere in a channel. The sidebar shows one
/// indicator per room, not one per conversation inside it.
export function busyInChannel<T extends QueueItem>(queues: Queues<T>, channelId: string): boolean {
  const prefix = `${channelId}:`
  return Object.entries(queues).some(([key, items]) => key.startsWith(prefix) && busy(items))
}
