import type { Box } from './box'
import type { ChannelReadState } from './types'

// Read counts and their ordering, in one owner.
//
// Three sources write the same counts: the WebSocket, a mark-read reply, and the
// resync snapshot. Any of them can be older than what is already shown, and the
// counts are the server's, so the client cannot repair a lost one by guessing.
//
// The rule is one version per key, bumped by EVERY change — application and
// removal alike. A writer captures its key's version before its request goes out
// and only lands if that version still holds. Bumping on socket events alone
// would leave a hole: with no event at all, a stale resync snapshot could still
// overwrite a newer applied reply.
//
// The map lives here too, not beside this. Splitting the counts from the
// bookkeeping that guards them is what let a logout clear one and leave the
// other, so they are reset together or not at all.
//
// The epoch is the account, captured when work is QUEUED so a request waiting
// behind another is dropped before it is ever dispatched. Keys are opaque:
// today they are channel IDs, and a per-conversation key needs nothing here.
export class ReadState {
  private counts: Box<Map<string, ChannelReadState>>
  private versions = new Map<string, number>()
  private inflight = new Map<string, Promise<unknown>>()
  private generation = 0

  constructor(counts: Box<Map<string, ChannelReadState>>) { this.counts = counts }

  get epoch() { return this.generation }
  get all() { return this.counts.get() }
  get(key: string) { return this.counts.get().get(key) }
  version(key: string) { return this.versions.get(key) ?? 0 }
  /// The versions to compare against after a long multi-key read such as resync.
  snapshot() { return new Map(this.versions) }

  private bump(key: string) { this.versions.set(key, this.version(key) + 1) }
  /// Whether a writer that captured this epoch and version still owns the key.
  private current(epoch: number, key: string, at: number) {
    return epoch === this.generation && at === this.version(key)
  }

  /// Apply state the server just pushed. Unconditional, and the newest thing this
  /// key has seen by definition.
  apply(state: ChannelReadState) {
    this.bump(state.channel_id)
    this.counts.set(new Map(this.counts.get()).set(state.channel_id, state))
  }

  /// Apply a delayed response, but only if the account and that one key's version
  /// are still the ones captured before the request went out.
  applyIf(state: ChannelReadState, epoch: number, at: number) {
    if (!this.current(epoch, state.channel_id, at)) return false
    this.apply(state)
    return true
  }

  /// Drop a key the server no longer reports. The version is bumped and KEPT:
  /// deleting it would hand the next request the same zero that a pending one is
  /// still holding, which is the token this whole mechanism turns on.
  private forget(key: string) {
    this.bump(key)
    const next = new Map(this.counts.get())
    next.delete(key)
    this.counts.set(next)
  }

  /// Run same-key work one at a time, so two responses for one key cannot reorder
  /// even with no socket attached. Different keys never wait on each other. The
  /// slot is registered SYNCHRONOUSLY, before waiting, so a caller arriving while
  /// this one is queued waits on this slot rather than on the same earlier
  /// promise; chaining against whatever happens to be in flight makes every
  /// waiter wake together and dispatch at once. `work` receives the version
  /// captured at DISPATCH, after the wait, which is the only correct moment to
  /// capture it. The marker is always released, so a failure never blocks that
  /// key's next read.
  async serialize<T>(key: string, work: (at: number) => Promise<T>): Promise<T | undefined> {
    const queued = this.generation
    const earlier = this.inflight.get(key)
    let settle!: () => void
    const slot = new Promise<void>((r) => { settle = r })
    this.inflight.set(key, slot)
    try {
      if (earlier) await earlier.catch(() => {})
      if (queued !== this.generation) return undefined
      return await work(this.version(key))
    } finally {
      settle()
      if (this.inflight.get(key) === slot) this.inflight.delete(key)
    }
  }

  /// Apply a whole resync snapshot, each key through its own queue and against
  /// the epoch and version the snapshot was taken at. A snapshot that comes back
  /// while an acknowledgement for that key is still in flight waits for it and
  /// then loses, whichever of the two the network happened to deliver first.
  /// Keys absent from the snapshot are dropped under the same rule, so a deleted
  /// channel's count cannot outlive it while a newer update still wins.
  async reconcile(rows: ChannelReadState[], epoch: number, versionsAt: Map<string, number>) {
    const present = new Set(rows.map((r) => r.channel_id))
    const gone = [...this.counts.get().keys()].filter((key) => !present.has(key))
    await Promise.all([
      ...rows.map((row) => this.serialize(row.channel_id, async () =>
        this.applyIf(row, epoch, versionsAt.get(row.channel_id) ?? 0))),
      ...gone.map((key) => this.serialize(key, async () => {
        if (this.current(epoch, key, versionsAt.get(key) ?? 0)) this.forget(key)
      })),
    ])
  }

  /// Logout or disposal. The counts go with the versions and the queue, in one
  /// synchronous step: nothing queued for the old account may dispatch, apply, or
  /// leave a number behind for whoever signs in next.
  reset() {
    this.versions.clear()
    this.inflight.clear()
    this.generation++
    this.counts.set(new Map())
  }
}
