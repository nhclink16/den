// All client state in one place, Svelte 5 runes. The server is the truth; this is a cache
// that the WebSocket keeps warm and a resync throws away.
import { sounds as playback } from './sounds'
import { receiveAlert } from './notify.svelte'
import type { SoundState, SoundPreferences } from './types'
import { Uploads } from './uploads.svelte'
import { Drafts, type Draft } from './drafts'
import { ReadState } from './read-state'
import { appendNew, byActivity } from './thread-order'
import { MessageFetches, applyRange, afterRange, beforeRange, cacheKey, type CacheId, type Mode, type Op } from './message-fetch'
import { themes } from './theme.svelte'
import type { Appearance, VoicePreferences } from './types'
import { objects } from './objects.svelte'
import type { ClientEvent, TerminalFrame, Settings } from './types'
import { call } from './call.svelte'
import { apiFor, setCsrf } from './api'
import { native, invoke, activeOrigin, setOrigin } from './native'
import { router } from './router.svelte'
import { cachedAppearance } from './theme-runtime'
import type { Jam, RoomJam, SpotifyAccount, MusicQueue, CallState, Category, Channel, ChannelReadState, Event, Message, NotificationPreferences, PresenceState, Reaction, Session, ThreadReadState, ThreadSummary, ThreadView, User } from './types'

const PREFS_KEY = 'den.layout'

export type Layout = { sidebar: boolean; members: boolean; sounds: boolean }

function loadLayout(): Layout {
  const fallback: Layout = { sidebar: true, members: true, sounds: false }
  try { return { ...fallback, ...JSON.parse(localStorage.getItem(PREFS_KEY) || '{}') } } catch { return fallback }
}

const PAGE = 50
const activeListeners = new Set<(event: Event) => void>()
const noSpotify = (): SpotifyAccount => ({ connection: 'unavailable', account_name: null, connected_at: null, expires_at: null })

export class Store {
  constructor(public origin: string) { this.api = apiFor(origin); this.uploads = new Uploads(origin); this.appearance = cachedAppearance(origin) }
  readonly uploads: Uploads
  /// Unsent text per conversation, independent of which composer is mounted.
  private draftEntries = $state.raw<Record<string, Draft>>({})
  readonly drafts = new Drafts({
    get: () => this.draftEntries,
    set: (v) => { this.draftEntries = v },
  })
  readonly api: ReturnType<typeof apiFor>
  appearance = $state<Appearance>(cachedAppearance())
  sounds = $state<SoundState | null>(null)
  async loadSounds() { this.sounds = await this.api.get<SoundState>('/users/me/sounds') }
  async saveSounds(value: SoundPreferences) { this.sounds = await this.api.put<SoundState>('/users/me/sounds', value) }
  voice = $state<VoicePreferences>({ microphones: {}, cameras: {} })
  private voiceWrites = Promise.resolve()
  private receiveVoice(value: VoicePreferences) {
    this.voice = value
    if (call.owner === this) void call.applyAV()
  }
  async saveVoice(patch: VoicePreferences) {
    // Serialize writes from this browser; the server merges entries from other devices.
    const write = this.voiceWrites.then(async () => {
      this.receiveVoice(await this.api.put<VoicePreferences>('/users/me/voice', patch))
    })
    this.voiceWrites = write.catch(() => {})
    return write
  }
  musicReceivedAt = new Map<string, number>()
  music = $state<Map<string, MusicQueue>>(new Map())
  receiveMusic(q: MusicQueue) {
    const current = this.music.get(q.room_id)
    if (current && (q.revision < current.revision || q.revision === current.revision && q.updated_at < current.updated_at)) return
    this.musicReceivedAt.set(q.room_id, Date.now()); this.music = new Map(this.music).set(q.room_id, q)
  }
  async loadMusic(room: string) { this.receiveMusic(await this.api.get<MusicQueue>(`/rooms/${room}/music`)) }
  /** One live Jam per room, keyed by channel. Absent means nobody started one. */
  jams = $state<Map<string, Jam>>(new Map())
  /** Bumped on every accepted write, so a slow poll cannot resurrect an ended Jam. */
  private jamSeq = new Map<string, number>()
  receiveJam(channelId: string, jam: Jam | null) {
    this.jamSeq.set(channelId, (this.jamSeq.get(channelId) ?? 0) + 1)
    const next = new Map(this.jams)
    if (jam) next.set(channelId, jam); else next.delete(channelId)
    this.jams = next
  }
  async loadJam(room: string) {
    const seq = (this.jamSeq.get(room) ?? 0) + 1
    this.jamSeq.set(room, seq)
    const { jam } = await this.api.get<RoomJam>(`/rooms/${room}/jam`)
    // A socket event or a newer read landed while this one was in flight.
    if (this.jamSeq.get(room) !== seq) return
    this.receiveJam(room, jam ?? null)
  }
  /** This account's Spotify link. `unavailable` until the server says otherwise. */
  spotify = $state<SpotifyAccount>(noSpotify())
  async loadSpotify() { this.spotify = await this.api.get<SpotifyAccount>('/users/me/spotify') }
  calls = $state<CallState[]>([])
  private connecting = false
  private generation = 0
  get active() { return !native || instances.active === this }
  get attention() { return this.channels.some(c => { const u = this.unread(c.id); return u.mention || (c.kind === 'dm' && u.count > 0) }) }
  private receiveAppearance(a: Appearance) {
    this.appearance = a
    if (native) localStorage.setItem(`den.appearance:${this.origin}`, JSON.stringify(a))
    if (this.active) themes.receive(a)
  }
  settings = $state<Settings>({ canvas_enabled: true, instance_name: 'Den' })
  private listeners = new Set<(event: Event) => void>()
  onEvent(fn: (event: Event) => void) { this.listeners.add(fn); return () => { this.listeners.delete(fn) } }
  sendEvent(event: ClientEvent) { if (this.ws?.readyState === WebSocket.OPEN) this.ws.send(JSON.stringify(event)) }
  sendTerminal(event: TerminalFrame) { if (this.ws?.readyState === WebSocket.OPEN) this.ws.send(new TextEncoder().encode(JSON.stringify(event))) }
  me = $state<User | null>(null)
  users = $state<Map<string, User>>(new Map())
  channels = $state<Channel[]>([])
  categories = $state<Category[]>([])
  messages = $state<Map<string, Message[]>>(new Map())
  readState = $state<Map<string, ChannelReadState>>(new Map())
  notif = $state<NotificationPreferences>({ mentions: true, dms: true, subscribed_channel_ids: [] })
  online = $state<Set<string>>(new Set())
  typing = $state<Map<string, Map<string, number>>>(new Map()) // channel -> user -> expiry
  layout = $state<Layout>(loadLayout())
  toast = $state('')
  connected = $state(false)
  ready = $state(false)
  loadingOlder = $state<Set<string>>(new Set())
  exhausted = $state<Set<string>>(new Set())
  /** Live alerts from the server, consumed by notify. */
  alerts = $state<Extract<Event, { type: 'notification' }>[]>([])
  private ws: WebSocket | null = null
  private backoff = 800
  private lastTyping = new Map<string, number>()

  channel(id: string) { return this.channels.find((c) => c.id === id) }
  user(id: string) { return this.users.get(id) }
  name(id: string) { const u = this.users.get(id); return u ? u.display_name || u.username : 'someone' }

  get textChannels() { return this.channels.filter((c) => c.kind !== 'dm').sort((a, b) => a.position - b.position) }
  get dms() { return this.channels.filter((c) => c.kind === 'dm') }

  dmTitle(c: Channel) {
    const others = (c.member_ids || []).filter((id) => id !== this.me?.id)
    return others.length ? others.map((id) => this.name(id)).join(', ') : 'Just you'
  }
  title(c: Channel) { return c.kind === 'dm' ? this.dmTitle(c) : c.name }

  unread(channelId: string) {
    const s = this.readState.get(channelId)
    return { count: s?.unread_count ?? 0, mention: (s?.mention_count ?? 0) > 0, lastRead: s?.last_read_id ?? '' }
  }
  get totalUnread() { let n = 0; for (const s of this.readState.values()) n += s.unread_count; return n }

  // Counts and their ordering both live in ReadState; these are only the reactive
  // cells it writes through. Counts are the server's — the client no longer
  // guesses zero, because a socket update carrying a NEWER count can arrive while
  // a read's response is still in flight, and applying that body afterwards
  // silently loses it.
  //
  // Two owners, one per kind of position, because ThreadReadState carries a
  // channel_id too: keying both by channel_id would collide a thread's position
  // with its own room's. They share one copy of the ordering rules.
  private reads = new ReadState<ChannelReadState>(
    { get: () => this.readState, set: (v) => { this.readState = v } },
    (s) => s.channel_id,
  )
  threadRead = $state<Map<string, ThreadReadState>>(new Map())
  private threadReads = new ReadState<ThreadReadState>(
    { get: () => this.threadRead, set: (v) => { this.threadRead = v } },
    (s) => s.thread_id,
  )

  // Conversation caches. Metadata gets the same guarded ownership as positions —
  // a delayed open-list page must not resurrect a thread a socket event already
  // resolved — so it goes through an owner keyed by thread ID.
  threadMeta = $state<Map<string, ThreadSummary>>(new Map())
  private threadMetas = new ReadState<ThreadSummary>(
    { get: () => this.threadMeta, set: (v) => { this.threadMeta = v } },
    (t) => t.id,
  )
  // Per-room display order, decided on room entry and then STABLE for the visit:
  // new entries append rather than resorting something under the pointer. It is
  // what this room has been told about, never a complete set — the lists are
  // filtered and paginated, so absence from a page means nothing at all.
  threadOrder = $state<Map<string, string[]>>(new Map())
  threadMessages = $state<Map<string, Message[]>>(new Map())
  threadExhausted = $state<Set<string>>(new Set())

  // Windows around an old target, and the roots a conversation panel shows.
  // These live HERE, not in the components that render them, because a message
  // has exactly one update path: a detached snapshot in a component would go on
  // showing an edited, reacted-to or deleted message forever.
  windows = $state<Map<string, Message[]>>(new Map())
  windowState = $state<Map<string, {
    loading: boolean; error: string; start: boolean; end: boolean
    failedEdge?: string; failedOlder?: boolean
  }>>(new Map())
  roots = $state<Map<string, Message>>(new Map())

  /// Acknowledge the ROOM through a marker the caller actually displayed. The
  /// caller passes it because the view knows what is on screen; the tail of the
  /// cache is not the same thing when the reader is scrolled back or looking at
  /// an old target.
  async markRead(channelId: string, displayed?: string) {
    // No acknowledgements while the Store is not ready: logout turns this off
    // before anything it awaits, so an effect that is still mounted cannot start
    // a read for an account that is going away.
    if (!this.ready) return
    // The marker is what is displayed NOW, and it stays that message for the
    // whole life of this acknowledgement. A read queued behind another must not
    // pick up messages that arrived while it waited: you never saw them, and in a
    // room you have since left you never will.
    const marker = displayed ?? this.messages.get(channelId)?.at(-1)?.id
    if (!marker || (this.reads.get(channelId)?.last_read_id ?? '') >= marker) return
    const epoch = this.reads.epoch
    await this.reads.serialize(channelId, async (at) => {
      // Only the CURSOR is re-read after the wait. The view re-runs this on every
      // message and every read state, so several calls can be queued on one key;
      // once one of them has acknowledged through this marker the rest have
      // nothing to say, and sending them anyway would be a request loop.
      if ((this.reads.get(channelId)?.last_read_id ?? '') >= marker) return
      try {
        // roots_only: this acknowledges the main conversation only. The flat
        // meaning is reserved for the explicit Mark all read below.
        const state = await this.api.put<ChannelReadState>(`/channels/${channelId}/read`, { message_id: marker, roots_only: true })
        // The request itself succeeded, so any earlier failure is over.
        if (epoch === this.reads.epoch) this.readError = ''
        this.reads.applyIf(state, epoch, at)
      } catch (e) {
        // Observable rather than swallowed; a resync still repairs the view. A
        // failure from the previous account must not surface in this one.
        if (epoch === this.reads.epoch) this.readError = (e as Error).message
      }
    })
  }
  /// Explicit Mark all read: the FLAT acknowledgement, which is what the server
  /// means by a read with no roots_only. It captures a tail first so a reply
  /// arriving afterwards stays unread, and it never short-circuits on the room
  /// cursor — the room can be current while threads are not.
  async markAllRead(channelId: string) {
    if (!this.ready) return
    const epoch = this.reads.epoch
    const [tail] = await this.api.get<Message[]>(`/channels/${channelId}/messages?limit=1`)
    if (!tail || epoch !== this.reads.epoch) return
    await this.reads.serialize(channelId, async (at) => {
      try {
        const state = await this.api.put<ChannelReadState>(`/channels/${channelId}/read`, { message_id: tail.id })
        if (epoch === this.reads.epoch) this.readError = ''
        this.reads.applyIf(state, epoch, at)
      } catch (e) {
        if (epoch === this.reads.epoch) this.readError = (e as Error).message
      }
    })
    // The flat read moved every thread position in this channel too. Without a
    // socket, nothing would tell the loaded threads that.
    if (!this.connected) await this.refreshThreadStates(channelId)
  }
  readError = $state('')
  private setRead(s: ChannelReadState) { this.reads.apply(s) }

  // --- conversations ---

  thread(id: string) { return this.threadMeta.get(id) }
  /// The conversation hanging off a root message, once the server has made one.
  /// A panel is identified by its root, so this is how it finds its thread ID.
  threadForRoot(rootId: string) {
    for (const t of this.threadMeta.values()) if (t.root_message_id === rootId) return t
    return undefined
  }
  threadState(id: string) { return this.threadRead.get(id) }
  /// Summaries for a room in this visit's order. `open` drops resolved entries;
  /// the strip shows those, and the panel can still show a resolved one.
  threadsIn(channelId: string, open = true) {
    return (this.threadOrder.get(channelId) || [])
      .map((id) => this.threadMeta.get(id))
      .filter((t): t is ThreadSummary => !!t && (!open || !t.resolved_at))
  }
  /// Unread replies this account can see in a thread, from its own position.
  threadUnread(id: string) {
    const s = this.threadRead.get(id)
    return { count: s?.unread_count ?? 0, mention: (s?.mention_count ?? 0) > 0, following: !!s?.following }
  }

  private remember(channelId: string, ids: string[]) {
    const seen = this.threadOrder.get(channelId) || []
    const next = appendNew(seen, ids)
    if (next === seen) return
    this.threadOrder = new Map(this.threadOrder).set(channelId, next)
  }
  /// Recompute the strip order. Called on room entry only.
  orderThreads(channelId: string) {
    const ids = byActivity(this.threadOrder.get(channelId) || [], (id) => this.threadMeta.get(id))
    this.threadOrder = new Map(this.threadOrder).set(channelId, ids)
  }

  /// One page of a channel's threads. Filtered and paginated, so it is applied
  /// per key and tells us nothing about threads it does not mention.
  async loadThreads(channelId: string, opts: { resolved?: boolean; unreadOnly?: boolean; before?: string; limit?: number } = {}) {
    const epoch = this.threadReads.epoch
    const positions = this.threadReads.snapshot()
    const metas = this.threadMetas.snapshot()
    const p = new URLSearchParams({ limit: String(opts.limit ?? 50) })
    if (opts.resolved !== undefined) p.set('resolved', String(opts.resolved))
    if (opts.unreadOnly) p.set('unread_only', 'true')
    if (opts.before) p.set('before', opts.before)
    const views = await this.api.get<ThreadView[]>(`/channels/${channelId}/threads?${p}`)
    if (epoch !== this.threadReads.epoch) return []
    await Promise.all([
      this.threadMetas.applyPage(views.map((v) => v.thread), epoch, metas),
      this.threadReads.applyPage(views.map((v) => v.read_state), epoch, positions),
    ])
    // The queues above are awaited, so the account can have turned over again.
    if (epoch !== this.threadReads.epoch) return []
    this.remember(channelId, views.map((v) => v.thread.id))
    return views
  }

  /// Metadata plus this account's position for one conversation. A 404 is the
  /// only thing that means "gone": anything else leaves the cache alone.
  async loadThread(id: string): Promise<ThreadView | undefined> {
    const epoch = this.threadReads.epoch
    const positions = this.threadReads.snapshot()
    const metas = this.threadMetas.snapshot()
    try {
      const view = await this.api.get<ThreadView>(`/threads/${id}`)
      if (epoch !== this.threadReads.epoch) return undefined
      // Through the SAME queue a mark-read uses. A GET that overtook a held
      // acknowledgement would otherwise bump the version and make the real
      // acknowledgement lose when it finally lands.
      await Promise.all([
        this.threadMetas.applyPage([view.thread], epoch, metas),
        this.threadReads.applyPage([view.read_state], epoch, positions),
      ])
      if (epoch !== this.threadReads.epoch) return undefined
      this.remember(view.thread.channel_id, [id])
      return view
    } catch (e) {
      // Only a confirmed 404 means gone, and then EVERYTHING about it goes:
      // replies and pagination flags too, not just metadata and position.
      if ((e as { status?: number }).status === 404 && epoch === this.threadReads.epoch) this.forgetThread(id)
      throw e
    }
  }

  /// Drop every cache belonging to one conversation.
  private forgetThread(id: string) {
    // Everything in flight for this conversation is cancelled synchronously, so
    // neither a response nor a finally can rebuild what is being dropped. That
    // INCLUDES its root: a root message has thread_id null, so a cancellation
    // keyed only on the thread would miss it entirely.
    const root = this.threadMeta.get(id)?.root_message_id
    this.fetches.cancel((r) =>
      (r.cache.cache === 'thread' && r.cache.threadId === id)
      || (r.cache.cache === 'window' && r.cache.conversation === id)
      || (!!root && r.cache.cache === 'root' && r.cache.rootId === root))
    // Every window belonging to this conversation, from either map: a window can
    // exist in one and not the other depending on where it failed.
    for (const key of [...this.windows.keys(), ...this.windowState.keys()]) {
      if (key.split(':')[0] === id) this.closeWindow(key)
    }
    this.takePaging(id)
    if (root && this.roots.has(root)) { const r = new Map(this.roots); r.delete(root); this.roots = r }
    this.threadMetas.forget(id)
    this.threadReads.forget(id)
    if (this.threadMessages.has(id)) { const m = new Map(this.threadMessages); m.delete(id); this.threadMessages = m }
    if (this.threadExhausted.has(id)) { const s = new Set(this.threadExhausted); s.delete(id); this.threadExhausted = s }
    for (const [channelId, ids] of this.threadOrder) {
      if (!ids.includes(id)) continue
      this.threadOrder = new Map(this.threadOrder).set(channelId, ids.filter((x) => x !== id))
    }
  }

  // Every message load goes through one registry: it records what arrived while
  // a request was open and decides whether that request is still wanted. There
  // are no count-capped journals or tombstones — an operation lives exactly as
  // long as the fetch that might need it.
  private fetches = new MessageFetches()
  /// Requests are identified by the CACHE they write and the account they belong
  /// to. Latest and older share a cache on purpose, so they contend.
  private request(cache: CacheId, mode: Mode) {
    return { account: this.threadReads.epoch, cache, mode }
  }
  private liveOp(op: Op) { this.fetches.record(op) }

  /// A replacement OWNS this cache's paging state, including the flag the
  /// request it just cancelled will never clear. That superseded request
  /// declines to clear it in its `finally`, and correctly so — by then the flag
  /// may belong to a newer older-page request. So the replacement clears it
  /// here, synchronously, next to the `begin` that cancelled the old one.
  ///
  /// Nothing else would. `exhausted` heals itself on the next tail; this flag
  /// does not, and while it is set MessageList shows a permanent spinner and
  /// refuses to load any more history for that conversation.
  private takePaging(key: string) {
    if (!this.loadingOlder.has(key)) return
    const s = new Set(this.loadingOlder)
    s.delete(key)
    this.loadingOlder = s
  }

  /// The latest replies. This is the one load that establishes a whole fresh
  /// contiguous tail, so an empty successful page really does clear what was
  /// there — that is how a reconnect repairs a deletion missed while offline.
  async loadThreadMessages(id: string) {
    const req = this.request({ cache: 'thread', threadId: id }, 'latest')
    const fetch = this.fetches.begin(req)
    this.takePaging(id)
    try {
      const page = await this.api.get<Message[]>(`/threads/${id}/messages?limit=${PAGE}`)
      const { ok, ops } = this.fetches.end(fetch)
      if (!ok) return
      this.threadMessages = new Map(this.threadMessages)
        .set(id, applyRange(this.threadMessages.get(id) || [], page, { all: true }, ops, req))
      // A fresh tail resets its own pagination.
      const done = new Set(this.threadExhausted)
      page.length < PAGE ? done.add(id) : done.delete(id)
      this.threadExhausted = done
    } catch (e) {
      // A failed request clears nothing.
      this.fetches.end(fetch)
      throw e
    }
  }
  async loadOlderThreadReplies(id: string) {
    const first = this.threadMessages.get(id)?.[0]
    if (!first || this.loadingOlder.has(id) || this.threadExhausted.has(id)) return
    const cache: CacheId = { cache: 'thread', threadId: id }
    if (this.fetches.replacing(cache)) return
    const req = this.request(cache, 'older')
    const fetch = this.fetches.begin(req)
    this.loadingOlder = new Set(this.loadingOlder).add(id)
    let mine = true
    try {
      const page = await this.api.get<Message[]>(`/threads/${id}/messages?limit=${PAGE}&before=${first.id}`)
      const { ok, ops } = this.fetches.end(fetch)
      mine = ok
      if (!ok) return
      // An older page proves something only about the span it returned.
      const range = beforeRange(page, first.id, PAGE)
      this.threadMessages = new Map(this.threadMessages)
        .set(id, applyRange(this.threadMessages.get(id) || [], page, range, ops, req))
      if (page.length < PAGE) this.threadExhausted = new Set(this.threadExhausted).add(id)
    } catch (e) {
      mine = this.fetches.end(fetch).ok
      throw e
    } finally {
      // An invalidated request must not clear a newer request's spinner.
      if (mine) { const s = new Set(this.loadingOlder); s.delete(id); this.loadingOlder = s }
    }
  }

  /// Acknowledge one conversation through a displayed marker. An empty thread
  /// uses its root, so a reply arriving between load and read stays unread.
  async markThreadRead(id: string, displayed?: string) {
    if (!this.ready) return
    const summary = this.threadMeta.get(id)
    const marker = displayed ?? this.threadMessages.get(id)?.at(-1)?.id ?? summary?.root_message_id
    if (!marker || (this.threadRead.get(id)?.last_read_id ?? '') >= marker) return
    const epoch = this.threadReads.epoch
    await this.threadReads.serialize(id, async (at) => {
      if ((this.threadRead.get(id)?.last_read_id ?? '') >= marker) return
      try {
        const state = await this.api.put<ThreadReadState>(`/threads/${id}/read`, { message_id: marker })
        if (epoch === this.threadReads.epoch) this.readError = ''
        this.threadReads.applyIf(state, epoch, at)
      } catch (e) {
        if (epoch === this.threadReads.epoch) this.readError = (e as Error).message
      }
    })
    // Reading a conversation changes its CHANNEL's total. With a socket the
    // server tells us; without one the badge would sit there lit with nothing
    // left to clear.
    await this.refreshChannelRead(this.threadRead.get(id)?.channel_id, epoch)
  }

  /// Re-read one channel's authoritative aggregate. Never computed locally.
  private async refreshChannelRead(channelId: string | undefined, epoch: number) {
    if (!channelId || this.connected || epoch !== this.threadReads.epoch) return
    const roomEpoch = this.reads.epoch
    const versions = this.reads.snapshot()
    const all = await this.api.get<ChannelReadState[]>('/users/me/read-state').catch(() => undefined)
    const mine = all?.find((s) => s.channel_id === channelId)
    if (!mine || roomEpoch !== this.reads.epoch) return
    await this.reads.applyPage([mine], roomEpoch, versions)
  }

  /// Follow starts from the current tail even for someone already following, so
  /// it never asks to be told about history. Unfollow keeps the position.
  async followThread(id: string, following: boolean) {
    const epoch = this.threadReads.epoch
    await this.threadReads.serialize(id, async (at) => {
      const state = await this.api.put<ThreadReadState>(`/threads/${id}/follow`, { following })
      this.threadReads.applyIf(state, epoch, at)
    })
    await this.refreshChannelRead(this.threadRead.get(id)?.channel_id, epoch)
  }

  /// Rename, resolve or reopen. The response is NOT newest by definition: a
  /// rename accepted and then held can come back after somebody else's Resolve
  /// has already arrived over the socket, and applying it would reopen the
  /// conversation. Same-thread mutations queue, and the response applies only if
  /// nothing newer landed and the account has not turned over.
  async updateThread(id: string, patch: { title?: string; resolved?: boolean }) {
    const epoch = this.threadMetas.epoch
    let summary: ThreadSummary | undefined
    await this.threadMetas.serialize(id, async (at) => {
      if (epoch !== this.threadMetas.epoch) return
      summary = await this.api.patch<ThreadSummary>(`/threads/${id}`, patch)
      this.threadMetas.applyIf(summary, epoch, at)
    })
    return summary
  }

  /// Without a socket, a mutation's effect on other loaded positions has to be
  /// fetched. Same guards as any other response.
  private async refreshThreadStates(channelId: string) {
    const loaded = this.threadsIn(channelId, false).map((t) => t.id)
    const epoch = this.threadReads.epoch
    const positions = this.threadReads.snapshot()
    const views = await Promise.all(loaded.map((id) =>
      this.api.get<ThreadView>(`/threads/${id}`).catch(() => undefined)))
    if (epoch !== this.threadReads.epoch) return
    await this.threadReads.applyPage(
      views.filter((v): v is ThreadView => !!v).map((v) => v.read_state), epoch, positions)
  }

  saveLayout(patch: Partial<Layout>) {
    this.layout = { ...this.layout, ...patch }
    localStorage.setItem(PREFS_KEY, JSON.stringify(this.layout))
  }
  async saveNotif(patch: Partial<NotificationPreferences>) {
    this.notif = await this.api.put<NotificationPreferences>('/users/me/notification-preferences', { ...this.notif, ...patch })
  }

  // --- session ---
  async login(username: string, password: string) {
    const s = await this.api.post<Session>('/auth/login', { username, password })
    if (native) { await invoke('session_set', { origin: this.origin, token: s.token }); instances.remember(this) }
    setCsrf(s.csrf_token); this.me = s.user
    await this.boot()
  }
  async register(username: string, password: string, invite: string, displayName?: string) {
    const display_name = displayName?.trim() || undefined
    const s = await this.api.post<Session>('/auth/register', { username, password, invite, display_name })
    if (native) { await invoke('session_set', { origin: this.origin, token: s.token }); instances.remember(this) }
    setCsrf(s.csrf_token); this.me = s.user
    await this.boot()
  }
  async logout() {
    // Everything that makes this Store usable is released SYNCHRONOUSLY, before
    // the first await. Leaving a call takes time, and during it an event queued
    // on the old socket, a reconnect scheduled by that socket closing, or a view
    // effect still mounted could otherwise refill the state just cleared.
    //
    // `generation` is the connection/account epoch the socket and the reconnect
    // loop already check, and `ready` is what the view and new acknowledgements
    // check; both are turned over here rather than at the end.
    const socket = this.ws
    this.generation++
    this.ready = false
    this.connected = false
    this.ws = null
    this.uploads.clear()
    this.drafts.clear()
    // Conversations go with everything else, in the same synchronous step.
    this.threadMetas.reset()
    this.threadReads.reset()
    this.threadOrder = new Map()
    this.threadMessages = new Map()
    this.windows = new Map()
    this.windowState = new Map()
    this.roots = new Map()
    this.threadExhausted = new Set()
    // Same orphan: a paging flag left set would wedge that conversation's
    // history for the NEXT account. `exhausted` is left alone deliberately —
    // the next latest tail recomputes it, this flag has no such repair.
    this.loadingOlder = new Set()
    this.jams = new Map()
    this.jamSeq.clear()
    this.spotify = noSpotify()
    // Logout cancels every open fetch, so no response and no finally can touch
    // the next account's state.
    this.fetches.clear()
    // The counts, their versions and anything queued against them go in one step,
    // so nothing from this account can reappear in the next.
    this.reads.reset()
    this.readError = ''
    socket?.close()
    if (call.origin === this.origin || !native) await call.leave()
    if (this.active) { call.snapshot([]); objects.active = null; objects.expanded = false; objects.presence = {} }
    try { await this.api.post('/auth/logout') } catch { /* already gone */ }
    if (native) await invoke('session_clear', { origin: this.origin })
    setCsrf(null); this.me = null
  }
  async resume(): Promise<boolean> {
    try { this.settings = await this.api.get<Settings>('/settings') } catch { /* Default name while offline. */ }
    try { this.me = await this.api.get<User>('/users/me'); await this.boot(); return true } catch { return false }
  }
  private async boot() { await this.resync(); this.ready = true; this.connect() }

  async resync() {
    // Captured before the requests go out, so anything applied while they are in
    // flight makes this snapshot stale for that key.
    const epoch = this.reads.epoch
    // The conversation owner keeps its own counter; they are reset together but
    // they are not the same number, so the follow-up gets the one it compares.
    const threadEpoch = this.threadReads.epoch
    const versionsAt = this.reads.snapshot()
    const [users, channels, categories, read, notif, presence, calls, settings, appearance, voice, sounds, spotify] = await Promise.all([
      this.api.get<User[]>('/users'),
      this.api.get<Channel[]>('/channels'),
      this.api.get<Category[]>('/categories'),
      this.api.get<ChannelReadState[]>('/users/me/read-state'),
      this.api.get<NotificationPreferences>('/users/me/notification-preferences'),
      this.api.get<PresenceState>('/presence'),
      this.api.get<CallState[]>('/calls'),
      this.api.get<Settings>('/settings'),
      this.api.get<Appearance>('/users/me/appearance'),
      this.api.get<VoicePreferences>('/users/me/voice'),
      this.api.get<SoundState>('/users/me/sounds'),
      // A new desktop client can still connect to an older Den server. Spotify
      // is optional there; a missing endpoint must not make the whole app fail.
      this.api.get<SpotifyAccount>('/users/me/spotify').catch(() => noSpotify()),
    ])
    // Everything below mutates this Store. A resync that was in flight across a
    // logout belongs to the account that asked for it, not to this one.
    if (epoch !== this.reads.epoch || threadEpoch !== this.threadReads.epoch) return
    this.receiveAppearance(appearance)
    this.sounds = sounds
    this.spotify = spotify
    this.receiveVoice(voice)
    this.settings = settings
    if (this.active) objects.presence = Object.fromEntries(presence.objects.map((o) => [o.id, o.user_ids]))
    this.users = new Map(users.map((u) => [u.id, u]))
    this.channels = channels
    this.categories = categories.sort((a, b) => a.position - b.position)
    // Each key goes through its own acknowledgement queue against the epoch and
    // version this snapshot was taken at, so it waits for an acknowledgement in
    // flight and then loses if it is stale — whichever of the two the network
    // delivered first. Channels the snapshot no longer reports are dropped under
    // the same rule rather than lingering as a count for a room that is gone.
    //
    // Deliberately NOT awaited. The server sends a `resync` event on connect, and
    // that handler runs inside the single WebSocket event chain: waiting here for
    // a slow acknowledgement on one channel would stop every event for every
    // channel until it came back. Counts arrive reactively, so nothing downstream
    // needs them settled before this returns.
    void this.reads.reconcile(read, epoch, versionsAt).catch(() => { /* a later resync repairs it */ })
    this.notif = notif
    this.online = new Set(presence.online_user_ids)
    this.calls = calls
    if (this.active) call.snapshot(calls)
    await Promise.all([...this.music.keys()].filter(id => channels.some(c => c.id === id)).map(id => this.loadMusic(id)))
    // A Jam may have started while the socket was down. Loading only keys already
    // in the map would miss that gap forever, so resync every visible room.
    await Promise.all(channels.map(channel => this.loadJam(channel.id).catch(() => {})))
    // Refresh the tail of channels we already had open so the view is current after a gap.
    await Promise.all([...this.messages.keys()].filter((id) => channels.some((c) => c.id === id)).map((id) => this.loadLatest(id)))
    // Deliberately NOT awaited, for the same reason the channel reconcile is
    // not: this runs inside handle('resync'), which runs inside the single
    // WebSocket event chain. Waiting here for one conversation's held
    // acknowledgement would stop every later frame, for every room.
    // The ORIGINAL epoch travels with it. Re-capturing inside the follow-up
    // would read whatever account is current by then, and a resync that waited
    // through a logout would go on to delete the new account's conversations.
    void this.refreshThreads(channels, threadEpoch).catch(() => { /* a later resync repairs it */ })
  }

  /// Bring loaded conversations back after a gap. The channel list IS
  /// authoritative, so a channel that is no longer in it takes its cached
  /// conversations with it; a failed request never means an empty list.
  private async refreshThreads(channels: Channel[], epoch: number) {
    if (epoch !== this.threadReads.epoch) return
    const live = new Set(channels.map((c) => c.id))
    // A personal position can arrive before any metadata, and it carries its own
    // channel_id: invalidate from that too, or a private conversation's state
    // survives losing access to the room it belongs to.
    for (const [id, summary] of this.threadMeta) if (!live.has(summary.channel_id)) this.forgetThread(id)
    for (const [id, state] of this.threadRead) if (!live.has(state.channel_id)) this.forgetThread(id)
    for (const channelId of [...this.threadOrder.keys()]) {
      if (live.has(channelId)) continue
      const next = new Map(this.threadOrder); next.delete(channelId); this.threadOrder = next
    }
    for (const id of [...this.threadMessages.keys()]) {
      if (this.threadMeta.has(id)) continue
      const next = new Map(this.threadMessages); next.delete(id); this.threadMessages = next
    }
    if (epoch !== this.threadReads.epoch) return
    // Refresh what is actually on display, then the replies we already hold.
    await Promise.all([...this.threadOrder.keys()]
      .filter((id) => live.has(id))
      .map((id) => this.loadThreads(id, { resolved: false }).catch(() => undefined)))
    if (epoch !== this.threadReads.epoch) return
    await Promise.all([...this.threadMessages.keys()].map((id) => this.loadThreadMessages(id).catch(() => undefined)))
    if (epoch !== this.threadReads.epoch) return
    await Promise.all([...this.threadMeta.keys()].map((id) => this.loadThread(id).catch(() => undefined)))
    // Roots and windows are displayed content too: a gap in the socket can have
    // changed them, and opening one earlier does not mean it never refreshes.
    if (epoch !== this.threadReads.epoch) return
    await Promise.all([...this.roots.keys()].map((id) => {
      const root = this.roots.get(id)
      return root ? this.loadRoot(id, root.channel_id).catch(() => undefined) : undefined
    }))
    if (epoch !== this.threadReads.epoch) return
    await Promise.all([...this.windows.keys()].map((key) => {
      const conversation = key.split(':')[0]!
      const target = key.slice(conversation.length + 1)
      const thread = this.threadMeta.get(conversation)
      // A bounded target-window replacement, which resets its outer paging.
      return this.openWindow(thread?.channel_id ?? conversation, thread ? conversation : undefined, target, true)
        .catch(() => undefined)
    }))
  }

  /// Where a message actually lives, for search results, quoted references, root
  /// badges, notification taps and pasted links. One resolver, so every entry
  /// point agrees. The destination is validated against the channel that was
  /// asked for: a cached private target must never be shown under another room.
  async locate(messageId: string, channelId: string): Promise<
    { channelId: string; threadId?: string; rootId?: string; message: Message } | undefined
  > {
    const message = await this.fetchMessage(messageId, channelId).catch(() => undefined)
    if (!message || message.channel_id !== channelId) return undefined
    if (!message.thread_id) return { channelId, message }
    // A reply knows its thread but not the root that opens it.
    const view = await this.loadThread(message.thread_id).catch(() => undefined)
    if (!view || view.thread.channel_id !== channelId) return undefined
    return { channelId, threadId: view.thread.id, rootId: view.thread.root_message_id, message }
  }

  // --- old-target windows ---
  //
  // A window is a contiguous slice around a message somebody linked to. It is
  // kept apart from the latest tail so a gap can never look like history that
  // has been read, but it is a real cache: edits, reactions and deletions reach
  // it through the same paths as everything else, and it pages outward on
  // demand with its own loading, error and end-of-history states.
  static readonly WINDOW = 25
  windowKey(channelId: string, threadId: string | undefined, target: string) {
    return `${threadId ?? channelId}:${target}`
  }
  windowAt(key: string) { return this.windows.get(key) }
  windowStatus(key: string) {
    return this.windowState.get(key) ?? { loading: false, error: '', start: false, end: false }
  }
  private setWindowState(key: string, part: Partial<{
    loading: boolean; error: string; start: boolean; end: boolean; failedEdge?: string; failedOlder?: boolean
  }>) {
    this.windowState = new Map(this.windowState).set(key, { ...this.windowStatus(key), ...part })
  }
  private path(channelId: string, threadId?: string) {
    return threadId
      ? { url: `/threads/${threadId}/messages`, roots: '' }
      : { url: `/channels/${channelId}/messages`, roots: '&roots_only=true' }
  }

  /// Open a window around `target`. The destination is validated: a message from
  /// another channel, or from another conversation, is not this window's target
  /// and is reported unavailable rather than quietly showing the latest list.
  async openWindow(channelId: string, threadId: string | undefined, target: string, refresh = false) {
    const key = this.windowKey(channelId, threadId, target)
    // `refresh` is how a reconnect re-reads a window it already holds; an
    // ordinary open still short-circuits so navigation does not refetch.
    if (!refresh && (this.windows.has(key) || this.windowStatus(key).loading)) return key
    const conversation = threadId ?? channelId
    const req = this.request({ cache: 'window', conversation, target }, 'open')
    const fetch = this.fetches.begin(req)
    this.setWindowState(key, { loading: true, error: '' })
    // A refresh already holds its anchor. Re-reading it is worth doing, but a
    // transient failure there must not throw away the window: the adjacent
    // pages are the point of the refresh, and the held copy is a usable anchor.
    const held = refresh ? this.windows.get(key)?.find((m) => m.id === target) : undefined
    try {
      const fetched = await this.fetchMessage(target, channelId).catch((e) => {
        if (held) return held
        throw e
      })
      if (!this.fetches.valid(fetch)) { this.fetches.end(fetch); return key }
      const usable = (m: Message | undefined) =>
        !!m && m.channel_id === channelId && (m.thread_id ?? undefined) === threadId
      const found = usable(fetched) ? fetched! : usable(held) ? held! : undefined
      if (!found) {
        this.fetches.end(fetch)
        this.setWindowState(key, { loading: false, error: 'That message is not in this conversation.' })
        return key
      }
      const { url, roots } = this.path(channelId, threadId)
      // before and after are mutually exclusive on both endpoints, so this is
      // two requests and the target itself joins them.
      const [older, newer] = await Promise.all([
        this.api.get<Message[]>(`${url}?limit=${Store.WINDOW}&before=${target}${roots}`),
        this.api.get<Message[]>(`${url}?limit=${Store.WINDOW}&after=${target}${roots}`),
      ])
      const { ok, ops } = this.fetches.end(fetch)
      if (!ok) return key
      // A window is bounded by what it actually fetched, and it is reset to that
      // bounded span rather than left holding outer pages nobody refreshed.
      const page = [...older, found, ...newer]
      const range = { from: page[0]!.id, fromInclusive: true, to: page.at(-1)!.id, toInclusive: true }
      this.windows = new Map(this.windows).set(key, applyRange([], page, range, ops, req))
      this.setWindowState(key, {
        loading: false, error: '',
        start: older.length < Store.WINDOW, end: newer.length < Store.WINDOW,
      })
      return key
    } catch (e) {
      // A failed lookup is a failure, not an empty window: it offers Retry and
      // never pretends to be loaded.
      if (this.fetches.end(fetch).ok) this.setWindowState(key, { loading: false, error: (e as Error).message })
      return key
    }
  }

  /// Page one bounded step outward.
  async extendWindow(key: string, channelId: string, threadId: string | undefined, older: boolean) {
    const have = this.windows.get(key)
    const status = this.windowStatus(key)
    if (!have?.length || status.loading || (older ? status.start : status.end)) return
    const edge = older ? have[0]!.id : have.at(-1)!.id
    // The SAME target representation as open, so the two contend on one cache.
    const conversation = threadId ?? channelId
    const target = key.slice(conversation.length + 1)
    const req = this.request({ cache: 'window', conversation, target }, 'extend')
    const fetch = this.fetches.begin(req)
    // Remembered so Retry resumes THIS edge and direction; calling openWindow
    // again would return immediately for a window that already exists.
    this.setWindowState(key, { loading: true, error: '', failedEdge: edge, failedOlder: older })
    try {
      const { url, roots } = this.path(channelId, threadId)
      const page = await this.api.get<Message[]>(
        `${url}?limit=${Store.WINDOW}&${older ? 'before' : 'after'}=${edge}${roots}`)
      const { ok, ops } = this.fetches.end(fetch)
      if (!ok) return
      // An extension changes only the span adjacent to the edge it asked about.
      const range = older ? beforeRange(page, edge, Store.WINDOW) : afterRange(page, edge, Store.WINDOW)
      this.windows = new Map(this.windows)
        .set(key, applyRange(this.windows.get(key) || [], page, range, ops, req))
      this.setWindowState(key, {
        loading: false, error: '', failedEdge: undefined,
        ...(older ? { start: page.length < Store.WINDOW } : { end: page.length < Store.WINDOW }),
      })
    } catch (e) {
      if (this.fetches.end(fetch).ok) this.setWindowState(key, { loading: false, error: (e as Error).message })
    }
  }

  /// Resume the extension that failed, at the same edge and direction.
  async retryWindow(key: string, channelId: string, threadId: string | undefined) {
    const status = this.windowStatus(key)
    if (status.failedEdge === undefined) return this.openWindow(channelId, threadId, key.split(':').slice(1).join(':'))
    return this.extendWindow(key, channelId, threadId, !!status.failedOlder)
  }

  closeWindow(key: string) {
    this.fetches.cancel((r) => r.cache.cache === 'window' && cacheKey(r.cache) === `window:${key}`)
    if (this.windows.has(key)) { const w = new Map(this.windows); w.delete(key); this.windows = w }
    if (this.windowState.has(key)) { const s = new Map(this.windowState); s.delete(key); this.windowState = s }
  }

  /// The root a conversation panel shows, cached here so edits and reactions
  /// reach it like any other displayed message.
  root(id: string) { return this.roots.get(id) }
  /// Fetch a root FROM THE SERVER. A cache hit is not an authoritative read: a
  /// refresh whose whole purpose is to find out what changed cannot answer
  /// itself out of the copy it is trying to check.
  async loadRoot(id: string, channelId: string) {
    const req = this.request({ cache: 'root', rootId: id }, 'latest')
    const fetch = this.fetches.begin(req)
    try {
      const found = await this.api.get<Message>(`/messages/${id}`).catch((e) => {
        const status = (e as { status?: number }).status
        if (status === 404 || status === 403) return undefined
        throw e
      })
      const { ok, ops } = this.fetches.end(fetch)
      if (!ok) return undefined
      // A root under the wrong channel is not this panel's root.
      if (!found || found.channel_id !== channelId) return undefined
      // Through the same rule: a change that arrived while this was open wins,
      // and a root fetch admits only its own id.
      const [settled] = applyRange([], [found], { all: true }, ops, req)
      if (!settled) { const r = new Map(this.roots); r.delete(id); this.roots = r; return undefined }
      this.roots = new Map(this.roots).set(id, settled)
      return settled
    } catch (e) {
      this.fetches.end(fetch)
      throw e
    }
  }

  // --- messages ---
  /// The room feed is the MAIN conversation: replies live in their thread.
  ///
  /// Through the same rule as everything else: it establishes a fresh contiguous
  /// tail, so an empty successful page really does clear the room, and a root
  /// that arrived while it was open is still there afterwards.
  async loadLatest(channelId: string) {
    const req = this.request({ cache: 'room', channelId }, 'latest')
    const fetch = this.fetches.begin(req)
    this.takePaging(channelId)
    try {
      const page = await this.api.get<Message[]>(`/channels/${channelId}/messages?limit=${PAGE}&roots_only=true`)
      const { ok, ops } = this.fetches.end(fetch)
      if (!ok) return
      this.messages = new Map(this.messages)
        .set(channelId, applyRange(this.messages.get(channelId) || [], page, { all: true }, ops, req))
      const done = new Set(this.exhausted)
      page.length < PAGE ? done.add(channelId) : done.delete(channelId)
      this.exhausted = done
    } catch (e) {
      this.fetches.end(fetch)
      throw e
    }
  }
  async loadOlder(channelId: string) {
    const first = this.messages.get(channelId)?.[0]
    if (!first || this.loadingOlder.has(channelId) || this.exhausted.has(channelId)) return
    const cache: CacheId = { cache: 'room', channelId }
    // A replacement for this same tail is already coming; an edge page now would
    // describe history it is about to discard.
    if (this.fetches.replacing(cache)) return
    const req = this.request(cache, 'older')
    const fetch = this.fetches.begin(req)
    this.loadingOlder = new Set(this.loadingOlder).add(channelId)
    let mine = true
    try {
      const page = await this.api.get<Message[]>(`/channels/${channelId}/messages?limit=${PAGE}&before=${first.id}&roots_only=true`)
      const { ok, ops } = this.fetches.end(fetch)
      mine = ok
      if (!ok) return
      this.messages = new Map(this.messages)
        .set(channelId, applyRange(this.messages.get(channelId) || [], page, beforeRange(page, first.id, PAGE), ops, req))
      if (page.length < PAGE) this.exhausted = new Set(this.exhausted).add(channelId)
    } catch (e) {
      mine = this.fetches.end(fetch).ok
      throw e
    } finally {
      if (mine) { const s = new Set(this.loadingOlder); s.delete(channelId); this.loadingOlder = s }
    }
  }
  /** A message by id, from cache or the server. Used for reply parents outside the loaded page. */
  /// A message by id, from cache or the server.
  ///
  /// `undefined` means CONFIRMED absent or denied. Anything else throws, because
  /// a view that cannot tell a 503 from a missing message will label a transient
  /// failure as an unavailable target and offer no way to retry. Passive callers
  /// that only want to decorate something catch and carry on.
  async fetchMessage(id: string, channelId: string): Promise<Message | undefined> {
    const hit = this.messages.get(channelId)?.find((m) => m.id === id)
      ?? [...this.threadMessages.values()].flat().find((m) => m.id === id)
      ?? this.roots.get(id)
    if (hit) return hit
    try {
      return await this.api.get<Message>(`/messages/${id}`)
    } catch (e) {
      const status = (e as { status?: number }).status
      if (status === 404 || status === 403) return undefined
      throw e
    }
  }

  async send(channelId: string, content: string, opts: { reply_to?: string; upload_ids?: string[]; thread_id?: string } = {}) {
    const epoch = this.generation
    const m = await this.api.post<Message>(`/channels/${channelId}/messages`, {
      content, reply_to: opts.reply_to ?? null, upload_ids: opts.upload_ids ?? [], thread_id: opts.thread_id ?? null,
    })
    // The server may well have accepted this write, and that stands: it was a
    // real message from the account that sent it. What must not follow is this
    // send moving a DIFFERENT account's cache or read cursor, which is what an
    // acknowledgement issued under the new epoch would do.
    if (epoch !== this.generation) return
    this.upsert(m)
    // Posting is not reading: an existing follower's unseen replies stay unread.
    if (!m.thread_id) this.markRead(channelId)
    return m
  }
  async edit(id: string, content: string) { this.upsert(await this.api.patch<Message>(`/messages/${id}`, { content })) }
  async remove(id: string, channelId: string) { await this.api.del(`/messages/${id}`); this.drop(id, channelId) }

  async react(m: Message, emoji: string) {
    const mine = m.reactions?.find((r) => r.emoji === emoji)?.user_ids.includes(this.me!.id)
    const reactions = mine
      ? await this.api.del<Reaction[]>(`/messages/${m.id}/reactions`, { emoji })
      : await this.api.put<Reaction[]>(`/messages/${m.id}/reactions`, { emoji })
    this.setReactions(m.channel_id, m.id, reactions)
  }
  private setReactions(channelId: string, id: string, reactions: Reaction[]) {
    // Recorded for every fetch in flight: a page that was snapshotted before
    // this must not put the old copy back when it lands.
    this.liveOp({ kind: 'reactions', id, reactions })
    // By message ID, across every cache that holds a copy: the room feed and any
    // loaded conversation. Reacting to a reply used to succeed on the server and
    // never appear in the panel.
    const list = this.messages.get(channelId)
    const i = list?.findIndex((x) => x.id === id) ?? -1
    if (list && i >= 0) this.messages = new Map(this.messages).set(channelId, list.with(i, { ...list[i]!, reactions }))
    for (const [key, list] of this.windows) {
      const w = list.findIndex((x) => x.id === id)
      if (w >= 0) this.windows = new Map(this.windows).set(key, list.with(w, { ...list[w]!, reactions }))
    }
    const root = this.roots.get(id)
    if (root) this.roots = new Map(this.roots).set(id, { ...root, reactions })
    for (const [threadId, replies] of this.threadMessages) {
      const j = replies.findIndex((x) => x.id === id)
      if (j < 0) continue
      this.threadMessages = new Map(this.threadMessages).set(threadId, replies.with(j, { ...replies[j]!, reactions }))
    }
  }

  async openDm(userIds: string[]) {
    const c = await this.api.post<Channel>('/dms', { member_ids: [...new Set([...userIds, this.me!.id])] })
    if (!this.channels.some((x) => x.id === c.id)) this.channels = [...this.channels, c]
    if (!this.messages.has(c.id)) await this.loadLatest(c.id)
    return c
  }

  async search(q: string, channelId?: string): Promise<Message[]> {
    const p = new URLSearchParams({ q, limit: '50' })
    if (channelId) p.set('channel_id', channelId)
    return this.api.get<Message[]>(`/search/messages?${p}`)
  }

  /// Route a message to every cache that should hold it. A reply belongs to its
  /// thread and never to the room feed; a root belongs to the room and also
  /// carries the thread summary the strip and its badge read.
  private upsert(m: Message, fromSocket = false) {
    this.liveOp({ kind: 'put', message: m })
    this.touchCopies(m)
    if (m.thread) {
      // Only an ordered socket payload is newest by definition. An accepted send
      // or edit can be held long enough for a rename or a Resolve to land first,
      // so its embedded summary only fills a gap it would otherwise leave.
      if (fromSocket || !this.threadMeta.has(m.thread.id)) this.threadMetas.apply(m.thread)
      this.remember(m.channel_id, [m.thread.id])
    }
    if (m.thread_id) {
      const replies = this.threadMessages.get(m.thread_id)
      if (replies) {
        const i = replies.findIndex((x) => x.id === m.id)
        this.threadMessages = new Map(this.threadMessages)
          .set(m.thread_id, i >= 0 ? replies.with(i, m) : [...replies, m])
      }
      return
    }
    const list = this.messages.get(m.channel_id)
    if (!list) return // not loaded; read state carries the unread count
    const i = list.findIndex((x) => x.id === m.id)
    this.messages = new Map(this.messages).set(m.channel_id, i >= 0 ? list.with(i, m) : [...list, m])
  }

  /// Every OTHER place a message can be on screen: an old-target window and a
  /// panel root. Windows are slices, so an existing entry is replaced but a new
  /// tail message is not appended into the middle of history.
  private touchCopies(m: Message) {
    for (const [key, list] of this.windows) {
      const i = list.findIndex((x) => x.id === m.id)
      if (i >= 0) this.windows = new Map(this.windows).set(key, list.with(i, m))
    }
    if (this.roots.has(m.id)) this.roots = new Map(this.roots).set(m.id, m)
  }
  /// Deletion events carry no thread ID, so every cached copy is searched by ID.
  private drop(id: string, channelId: string) {
    // Recorded for every fetch in flight: the event carries no conversation and
    // the message may not be cached at all, so a page already in flight would
    // otherwise deliver it back.
    this.liveOp({ kind: 'delete', id })
    for (const [key, list] of this.windows) {
      if (!list.some((x) => x.id === id)) continue
      this.windows = new Map(this.windows).set(key, list.filter((x) => x.id !== id))
    }
    if (this.roots.has(id)) { const r = new Map(this.roots); r.delete(id); this.roots = r }
    const list = this.messages.get(channelId)
    if (list?.some((x) => x.id === id)) {
      this.messages = new Map(this.messages).set(channelId, list.filter((x) => x.id !== id))
    }
    for (const [threadId, replies] of this.threadMessages) {
      if (!replies.some((x) => x.id === id)) continue
      this.threadMessages = new Map(this.threadMessages).set(threadId, replies.filter((x) => x.id !== id))
    }
  }

  // --- realtime ---
  private async connect() {
    // `ready` is false from the first line of logout until the next boot, so a
    // reconnect cannot open a socket for an account that is being torn down.
    if (this.ws || this.connecting || !this.me || !this.ready) return
    this.connecting = true
    const generation = this.generation
    let url = `${this.origin.replace(/^http/, 'ws')}/ws?sounds=true`
    try {
      if (native) {
        const ticket = await this.api.post<import('./types').WsTicket>('/auth/ws-ticket')
        url += `&ticket=${encodeURIComponent(ticket.ticket)}`
      }
      if (generation !== this.generation || !this.me) return
    } catch {
      if (generation === this.generation && this.me) setTimeout(() => void this.connect(), this.backoff)
      this.backoff = Math.min(this.backoff * 2, 15_000)
      return
    } finally { this.connecting = false }
    url += `${url.includes('?') ? '&' : '?'}music=true&jam=true`
    const ws = new WebSocket(url)
    this.ws = ws
    ws.onopen = () => { this.connected = true; this.backoff = 800 }
    ws.binaryType = 'arraybuffer'
    let events = Promise.resolve()
    ws.onmessage = (e) => {
      // Events already queued when the account changed are dropped rather than
      // repopulating state that a logout deliberately cleared. `generation` is
      // the connection/account epoch this socket was opened for.
      events = events.then(() => generation === this.generation
        ? this.handle(JSON.parse(typeof e.data === 'string' ? e.data : new TextDecoder().decode(e.data)) as Event)
        : undefined)
        .catch(() => { ws.close() })
    }
    ws.onclose = () => {
      if (generation !== this.generation) return   // a socket from a past account
      this.connected = false; this.ws = null
      if (!this.me || !this.ready) return
      setTimeout(() => this.connect(), this.backoff)
      this.backoff = Math.min(this.backoff * 2, 15_000)
    }
  }

  /** Tell the room you're typing. The server rate-limits to one per two seconds per channel. */
  /// Scoped to the conversation being typed in, throttle included: a thread's
  /// typing must not read as another main-room update. A panel with no saved
  /// thread yet says nothing rather than claiming the room.
  sendTyping(channelId: string, threadId?: string, unsaved = false) {
    if (unsaved) return
    const key = threadId ? `t:${threadId}` : channelId
    const now = Date.now()
    if (now - (this.lastTyping.get(key) || 0) < 2000 || this.ws?.readyState !== WebSocket.OPEN) return
    this.lastTyping.set(key, now)
    this.ws.send(JSON.stringify({ type: 'typing', channel_id: channelId, thread_id: threadId ?? null }))
  }

  private async handle(ev: Event) {
    for (const fn of this.listeners) fn(ev)
    if (this.active) for (const fn of activeListeners) fn(ev)
    switch (ev.type) {
      case 'music_queue_updated': this.receiveMusic(ev.queue); break
      case 'jam_updated': this.receiveJam(ev.channel_id, ev.jam ?? null); break
      case 'spotify_account_updated': this.spotify = ev.account; break
      case 'sounds_updated': void this.loadSounds(); break
      case 'voice_preferences_updated': this.receiveVoice(ev.preferences); break
      case 'appearance_updated': this.receiveAppearance(ev.appearance); break
      case 'settings_updated': this.settings = ev.settings; break
      case 'object_presence': if (this.active) objects.presence = { ...objects.presence, [ev.id]: ev.user_ids }; break
      case 'resync': if (this.ready) await this.resync(); break
      case 'message_created':
      case 'message_edited': {
        const { type: _t, ...m } = ev
        if (!this.channels.some((c) => c.id === m.channel_id)) await this.resync()
        else this.upsert(m as Message, true)
        if (ev.type === 'message_created') {
          // They stopped typing IN THAT CONVERSATION. Clearing the room key for a
          // thread reply would leave the thread indicator running forever and
          // silence the room's instead.
          const key = m.thread_id ? `t:${m.thread_id}` : m.channel_id
          const chan = this.typing.get(key)
          if (chan?.has(m.author_id)) { const c = new Map(chan); c.delete(m.author_id); this.typing = new Map(this.typing).set(key, c) }
        }
        break
      }
      case 'message_deleted':
        if (this.active && objects.active?.message_id === ev.id) { objects.active = null; objects.expanded = false }
        this.drop(ev.id, ev.channel_id); break
      case 'reactions_updated': this.setReactions(ev.channel_id, ev.message_id, ev.reactions); break
      case 'read_state_updated': this.setRead(ev.state); break
      case 'thread_updated': {
        this.threadMetas.apply(ev.thread)
        this.remember(ev.thread.channel_id, [ev.thread.id])
        break
      }
      // Personal position for one conversation. It may arrive before any
      // metadata; the position is kept either way and metadata is fetched only
      // when something actually needs to display it.
      case 'thread_read_state_updated': this.threadReads.apply(ev.state); break
      case 'notification_preferences_updated': this.notif = ev.preferences; break
      case 'notification': receiveAlert(this, ev); this.alerts = [...this.alerts, ev].slice(-100); if (native) window.dispatchEvent(new CustomEvent('den-alert', { detail: { origin: this.origin, alert: ev } })); break
      case 'call_state': this.calls = [...this.calls.filter(c => c.channel_id !== ev.channel_id), ev]; if (this.active) call.receive(ev); if (!this.channel(ev.channel_id)) await this.resync(); break
      case 'presence': {
        const s = new Set(this.online); ev.online ? s.add(ev.user_id) : s.delete(ev.user_id); this.online = s
        break
      }
      case 'typing': {
        if (ev.user_id === this.me?.id) break
        // Keyed by conversation: a thread's typing is not a room update.
        const key = ev.thread_id ? `t:${ev.thread_id}` : ev.channel_id
        const chan = new Map(this.typing.get(key) || [])
        chan.set(ev.user_id, Date.now() + 5000)
        this.typing = new Map(this.typing).set(key, chan)
        setTimeout(() => { this.typing = new Map(this.typing) }, 5100) // re-evaluate expiries
        break
      }
    }
  }

  typingNames(channelId: string, threadId?: string): string[] {
    const now = Date.now()
    const key = threadId ? `t:${threadId}` : channelId
    return [...(this.typing.get(key) || [])].filter(([, t]) => t > now).map(([id]) => this.name(id))
  }
}

class Instances {
  stores = $state<Store[]>([])
  active = $state.raw<Store>(new Store(activeOrigin()))
  adding = $state(false)
  invite = $state('')
  url = $state('')
  get all() { return native ? this.stores.filter(s => s.me) : [this.active] }
  get totalUnread() { return this.all.reduce((n, s) => n + s.totalUnread, 0) }
  get attention() { return this.all.some(s => s !== this.active && s.attention) }
  async resume() {
    if (!native) return this.active.resume()
    const origins = await invoke<string[]>('instances_get').catch(() => [])
    this.stores = [...new Set([...origins, this.active.origin])].map(o => o === this.active.origin ? this.active : new Store(o))
    await Promise.all(this.stores.map(s => s.resume()))
    if (!this.active.me && this.all[0]) this.select(this.all[0])
    return !!this.active.me
  }
  remember(s: Store) {
    if (!this.stores.includes(s)) this.stores = [...this.stores, s]
    void invoke('instances_set', { origins: this.stores.map(s => s.origin) })
  }
  select(s: Store) {
    objects.active = null; objects.expanded = false; objects.presence = {}
    this.active = s; setOrigin(s.origin)
    themes.draft = null; themes.receive(s.appearance, true); call.snapshot(s.calls)
    window.dispatchEvent(new CustomEvent('den-instance'))
    router.go(s.me ? '/' : '/login')
  }
  async remove(s: Store) {
    await s.logout()
    this.stores = this.stores.filter(x => x !== s)
    await invoke('instances_set', { origins: this.stores.map(s => s.origin) })
    if (s === this.active) this.select(this.all[0] || new Store('https://denchat.app'))
  }
  add(url = '', invite = '') { this.url = url; this.invite = invite; this.adding = true }
  key(e: KeyboardEvent) {
    if (!native || !(e.ctrlKey || e.metaKey) || !e.shiftKey) return
    const n = /^Digit[1-9]$/.test(e.code) ? Number(e.code.slice(-1)) - 1 : e.code === 'BracketRight' ? (this.all.indexOf(this.active) + 1) % this.all.length : -1
    if (this.all[n]) { e.preventDefault(); this.select(this.all[n]) }
  }
}
export const instances = new Instances()
// Existing views read the active store; an async operation captures its instance at invocation.
export const store = new Proxy({} as Store, {
  get(_target, key: keyof Store) { if (key === 'onEvent') return (fn: (event: Event) => void) => { activeListeners.add(fn); return () => activeListeners.delete(fn) }; const s = instances.active; const value = s[key]; return typeof value === 'function' ? value.bind(s) : value },
  set(_target, key, value) { Reflect.set(instances.active, key, value); return true },
})

window.addEventListener('den-sound-event', event => {
  const { origin, sound } = (event as CustomEvent<{ origin: string; sound: import('./types').SoundEvent }>).detail
  const owner = instances.all.find(s => s.origin === origin)
  if (owner) void playback.play(sound, owner)
})
