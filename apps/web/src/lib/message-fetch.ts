import type { Message, Reaction } from './types'

// One rule for every message load: room latest, room older, thread latest,
// thread older, a root, and a window around an old target. Each is a snapshot of
// a moment that has already passed by the time it arrives, so each records what
// happened while it was open and replays it afterwards.
//
// This is deliberately not a cache framework. It is a registry of the fetches
// currently open, the operations that arrived during them, and one function that
// decides what a page is allowed to say.

export type Op =
  | { kind: 'put'; message: Message }
  // Reactions and deletions are addressed by message id, which is unique across
  // the instance. They carry no conversation on the wire and need none: they
  // patch or remove that id wherever it is held, including a message the fetched
  // page itself just introduced.
  | { kind: 'reactions'; id: string; reactions: Reaction[] }
  | { kind: 'delete'; id: string }

/// WHICH CACHE a request writes. This is the identity that decides ordering and
/// cancellation, and it is deliberately separate from the operation mode below:
/// a latest load and an older page write the SAME tail, so they must contend
/// with each other rather than look like two independent requests.
///
/// A room and a thread can never collide, because they are different variants,
/// and a thread's identity is its thread id alone — so it does not change when
/// channel metadata finally arrives.
export type CacheId =
  | { cache: 'room'; channelId: string }
  | { cache: 'thread'; threadId: string }
  | { cache: 'root'; rootId: string }
  | { cache: 'window'; conversation: string; target: string }

export const cacheKey = (c: CacheId): string =>
  c.cache === 'room' ? `room:${c.channelId}`
  : c.cache === 'thread' ? `thread:${c.threadId}`
  : c.cache === 'root' ? `root:${c.rootId}`
  : `window:${c.conversation}:${c.target}`

/// HOW the cache is written. `latest` and `open` replace what is there; `older`
/// and `extend` only add at an edge.
export type Mode = 'latest' | 'older' | 'open' | 'extend'
const replaces = (m: Mode) => m === 'latest' || m === 'open'

export type Request = { account: number; cache: CacheId; mode: Mode }

/// The span of message ids a response actually proves something about. `all` is
/// the one case where a response really does replace a whole list.
export type Range =
  | { all: true }
  | { from?: string; to?: string; fromInclusive?: boolean; toInclusive?: boolean }

const within = (id: string, r: Range): boolean => {
  if ('all' in r) return true
  if (r.from !== undefined && (r.fromInclusive ? id < r.from : id <= r.from)) return false
  if (r.to !== undefined && (r.toInclusive ? id > r.to : id >= r.to)) return false
  return true
}

/// Coverage for a `before=X` page.
///
/// An EMPTY response is a SHORT page: it proves there is nothing below X at all,
/// so it establishes that whole end and stale entries inside it go. A full page
/// proves nothing older than its own first id.
export function beforeRange(page: Message[], x: string, limit: number): Range {
  if (page.length >= limit) return { from: page[0]!.id, fromInclusive: true, to: x }
  return { to: x }
}
/// Mirror of the above for `after=X`.
export function afterRange(page: Message[], x: string, limit: number): Range {
  if (page.length >= limit) return { from: x, to: page.at(-1)!.id, toInclusive: true }
  return { from: x }
}

export class MessageFetches {
  private seq = 0
  private open = new Map<number, { req: Request; key: string; ops: Op[] }>()

  /// Register a fetch and start recording.
  ///
  /// A replacement CANCELS the older open requests for the same cache as it
  /// starts, synchronously. There is deliberately no lasting record of which
  /// request was newest: a map like that gets pruned when the newer request
  /// finishes, and quietly makes a superseded one valid again.
  begin(req: Request) {
    const key = cacheKey(req.cache)
    if (replaces(req.mode)) {
      for (const [id, entry] of this.open) if (entry.key === key) this.open.delete(id)
    }
    const id = ++this.seq
    this.open.set(id, { req, key, ops: [] })
    return id
  }

  /// Still wanted? Cancelled and superseded requests are simply not open.
  valid(id: number) { return this.open.has(id) }

  /// Is a replacement for this cache in flight? An edge page dispatched now
  /// would describe history the replacement is about to discard.
  replacing(cache: CacheId) {
    const key = cacheKey(cache)
    for (const entry of this.open.values()) if (entry.key === key && replaces(entry.req.mode)) return true
    return false
  }

  /// Record a live change into every open fetch. Filtering happens at replay,
  /// because only the target cache knows what it may admit.
  record(op: Op) { for (const entry of this.open.values()) entry.ops.push(op) }

  /// Finish with a fetch. Validity is captured BEFORE consuming it, so a caller
  /// cannot mistake "it was still registered" for "it was never cancelled".
  end(id: number) {
    const ok = this.open.has(id)
    const entry = this.open.get(id)
    this.open.delete(id)
    return { ok, ops: entry?.ops ?? [], req: entry?.req }
  }

  /// Invalidate everything matching, synchronously: a confirmed removal, a
  /// closed window, a logout. Their responses and their finally blocks both
  /// check `valid` and do nothing.
  cancel(match: (req: Request) => boolean) {
    for (const [id, entry] of this.open) if (match(entry.req)) this.open.delete(id)
  }
  clear() { this.open.clear() }
}

/// Apply one response to one cache.
///
/// The page is authoritative ONLY inside the range it covers: entries in range
/// and absent from it are gone, entries present replace. Nothing outside the
/// range is touched, so a gap is never presented as loaded. Then the operations
/// that arrived while the request was open replay in order, because they are
/// newer than the snapshot by definition.
export function applyRange(
  existing: Message[],
  page: Message[],
  range: Range,
  ops: Op[],
  req: Request,
): Message[] {
  const byId = new Map(existing.map((m) => [m.id, m]))
  for (const m of existing) if (within(m.id, range)) byId.delete(m.id)
  for (const m of page) byId.set(m.id, m)

  // What this cache may ADMIT that it did not already hold. A latest tail takes
  // new arrivals; an edge page or a window takes only ids inside the span it
  // covered, so a far-newer reply is never appended across an unloaded gap; a
  // root takes only its own message.
  const admits = (m: Message) => {
    if (req.cache.cache === 'root') return m.id === req.cache.rootId
    if (req.mode === 'latest') return true
    return within(m.id, range)
  }
  // Which conversation an arrival belongs to.
  const mine = (m: Message) => {
    switch (req.cache.cache) {
      case 'thread': return m.thread_id === req.cache.threadId
      case 'room': return m.channel_id === req.cache.channelId && !m.thread_id
      case 'root': return m.id === req.cache.rootId
      case 'window': return (m.thread_id ?? m.channel_id) === req.cache.conversation
    }
  }

  for (const op of ops) {
    if (op.kind === 'delete') { byId.delete(op.id); continue }
    if (op.kind === 'reactions') {
      const held = byId.get(op.id)
      if (held) byId.set(op.id, { ...held, reactions: op.reactions })
      continue
    }
    const m = op.message
    if (!mine(m)) continue
    if (byId.has(m.id) || admits(m)) byId.set(m.id, m)
  }
  return [...byId.values()].sort((a, b) => a.id.localeCompare(b.id))
}
