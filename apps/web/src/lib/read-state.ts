import type { Box } from './box'

// Read positions and their ordering, in one owner per kind of conversation.
//
// Three sources write the same state: the WebSocket, a mark-read reply, and a
// refresh. Any of them can be older than what is already shown, and the counts
// are the server's, so the client cannot repair a lost one by guessing.
//
// The rule is one version per key, bumped by EVERY change — application and
// removal alike. A writer captures its key's version before its request goes out
// and only lands if that version still holds. Bumping on socket events alone
// would leave a hole: with no event at all, a stale refresh could still
// overwrite a newer applied reply.
//
// The map lives here too, not beside this. Splitting the state from the
// bookkeeping that guards it is what let a logout clear one and leave the other,
// so they are reset together or not at all.
//
// The epoch is the account, captured when work is QUEUED so a request waiting
// behind another is dropped before it is ever dispatched.
//
// Keys are NOT opaque to the caller: each owner is told how to key its own state
// type. That matters because ThreadReadState carries a channel_id as well as its
// thread_id, so one shared owner keyed on channel_id would quietly collide a
// thread's position with its own room's. The Store instantiates one owner per
// kind instead — channels keyed by channel_id, threads by thread_id — and both
// share this single copy of the ordering rules.
export class ReadState<S> {
  private states: Box<Map<string, S>>
  private keyOf: (state: S) => string
  private versions = new Map<string, number>()
  private inflight = new Map<string, Promise<unknown>>()
  private generation = 0

  constructor(states: Box<Map<string, S>>, keyOf: (state: S) => string) {
    this.states = states
    this.keyOf = keyOf
  }

  get epoch() { return this.generation }
  get all() { return this.states.get() }
  get(key: string) { return this.states.get().get(key) }
  version(key: string) { return this.versions.get(key) ?? 0 }
  /// The versions to compare against after a long multi-key read.
  snapshot() { return new Map(this.versions) }

  private bump(key: string) { this.versions.set(key, this.version(key) + 1) }
  /// Whether a writer that captured this epoch and version still owns the key.
  private current(epoch: number, key: string, at: number) {
    return epoch === this.generation && at === this.version(key)
  }

  /// Apply state the server just pushed. Unconditional, and the newest thing this
  /// key has seen by definition.
  apply(state: S) {
    const key = this.keyOf(state)
    this.bump(key)
    this.states.set(new Map(this.states.get()).set(key, state))
  }

  /// Apply a delayed response, but only if the account and that one key's version
  /// are still the ones captured before the request went out.
  applyIf(state: S, epoch: number, at: number) {
    if (!this.current(epoch, this.keyOf(state), at)) return false
    this.apply(state)
    return true
  }

  /// Apply a PAGE of states: each key guarded against its own captured version,
  /// and nothing inferred about keys the page does not mention. Filtered and
  /// paginated lists come through here. Deleting from a partial page would throw
  /// away the unread and follow state of everything it happened to exclude.
  applyPage(states: S[], epoch: number, versionsAt: Map<string, number>) {
    return Promise.all(states.map((state) => {
      const key = this.keyOf(state)
      return this.serialize(key, async () => this.applyIf(state, epoch, versionsAt.get(key) ?? 0))
    }))
  }

  /// Drop a key. The version is bumped and KEPT: deleting it would hand the next
  /// request the same zero that a pending one is still holding, which is the
  /// token this whole mechanism turns on.
  forget(key: string) {
    this.bump(key)
    const next = new Map(this.states.get())
    next.delete(key)
    this.states.set(next)
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

  /// Apply an AUTHORITATIVE and COMPLETE snapshot: keys absent from it are gone,
  /// dropped under the same version guard. Only the channel list qualifies —
  /// the server returns every channel the account can see in one response. Do not
  /// reach for this with a filtered or paginated list; use applyPage.
  async reconcile(states: S[], epoch: number, versionsAt: Map<string, number>) {
    const present = new Set(states.map((s) => this.keyOf(s)))
    const gone = [...this.states.get().keys()].filter((key) => !present.has(key))
    await Promise.all([
      this.applyPage(states, epoch, versionsAt),
      ...gone.map((key) => this.serialize(key, async () => {
        if (this.current(epoch, key, versionsAt.get(key) ?? 0)) this.forget(key)
      })),
    ])
  }

  /// Logout or disposal. The state goes with the versions and the queue, in one
  /// synchronous step: nothing queued for the old account may dispatch, apply, or
  /// leave a number behind for whoever signs in next.
  reset() {
    this.versions.clear()
    this.inflight.clear()
    this.generation++
    this.states.set(new Map())
  }
}
