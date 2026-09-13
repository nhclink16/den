// All client state in one place, Svelte 5 runes. The server is the truth; this is a cache
// that the WebSocket keeps warm and a resync throws away.
import { objects } from './objects.svelte'
import type { ClientEvent, TerminalFrame, Settings } from './types'
import { call } from './call.svelte'
import { api, setCsrf } from './api'
import type { CallState, Category, Channel, ChannelReadState, Event, Message, NotificationPreferences, PresenceState, Reaction, Session, User } from './types'

const PREFS_KEY = 'den.layout'

export type Layout = { sidebar: boolean; members: boolean; sounds: boolean }

function loadLayout(): Layout {
  const fallback: Layout = { sidebar: true, members: true, sounds: false }
  try { return { ...fallback, ...JSON.parse(localStorage.getItem(PREFS_KEY) || '{}') } } catch { return fallback }
}

const PAGE = 50

class Store {
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

  async markRead(channelId: string) {
    const last = this.messages.get(channelId)?.at(-1)?.id
    const cur = this.readState.get(channelId)
    if (!last || (cur && cur.last_read_id && cur.last_read_id >= last && cur.unread_count === 0)) return
    // Optimistic: the server confirms with read_state_updated.
    this.setRead({ channel_id: channelId, last_read_id: last, unread_count: 0, mention_count: 0, notification_count: 0 })
    try { this.setRead(await api.put<ChannelReadState>(`/channels/${channelId}/read`, { message_id: last })) } catch { /* resync will fix it */ }
  }
  private setRead(s: ChannelReadState) { this.readState = new Map(this.readState).set(s.channel_id, s) }

  saveLayout(patch: Partial<Layout>) {
    this.layout = { ...this.layout, ...patch }
    localStorage.setItem(PREFS_KEY, JSON.stringify(this.layout))
  }
  async saveNotif(patch: Partial<NotificationPreferences>) {
    this.notif = await api.put<NotificationPreferences>('/users/me/notification-preferences', { ...this.notif, ...patch })
  }

  // --- session ---
  async login(username: string, password: string) {
    const s = await api.post<Session>('/auth/login', { username, password })
    setCsrf(s.csrf_token); this.me = s.user
    await this.boot()
  }
  async register(username: string, password: string, invite: string) {
    const s = await api.post<Session>('/auth/register', { username, password, invite })
    setCsrf(s.csrf_token); this.me = s.user
    await this.boot()
  }
  async logout() {
    await call.leave(); call.snapshot([]); objects.active = null; objects.expanded = false; objects.presence = {}
    try { await api.post('/auth/logout') } catch { /* already gone */ }
    setCsrf(null); this.ws?.close(); this.me = null; this.ready = false
  }
  async resume(): Promise<boolean> {
    try { this.settings = await api.get<Settings>('/settings') } catch { /* Default name while offline. */ }
    try { this.me = await api.get<User>('/users/me'); await this.boot(); return true } catch { return false }
  }
  private async boot() { await this.resync(); this.ready = true; this.connect() }

  async resync() {
    const [users, channels, categories, read, notif, presence, calls, settings] = await Promise.all([
      api.get<User[]>('/users'),
      api.get<Channel[]>('/channels'),
      api.get<Category[]>('/categories'),
      api.get<ChannelReadState[]>('/users/me/read-state'),
      api.get<NotificationPreferences>('/users/me/notification-preferences'),
      api.get<PresenceState>('/presence'),
      api.get<CallState[]>('/calls'),
      api.get<Settings>('/settings'),
    ])
    this.settings = settings
    objects.presence = Object.fromEntries(presence.objects.map((o) => [o.id, o.user_ids]))
    this.users = new Map(users.map((u) => [u.id, u]))
    this.channels = channels
    this.categories = categories.sort((a, b) => a.position - b.position)
    this.readState = new Map(read.map((s) => [s.channel_id, s]))
    this.notif = notif
    this.online = new Set(presence.online_user_ids)
    call.snapshot(calls)
    // Refresh the tail of channels we already had open so the view is current after a gap.
    await Promise.all([...this.messages.keys()].filter((id) => channels.some((c) => c.id === id)).map((id) => this.loadLatest(id)))
  }

  // --- messages ---
  async loadLatest(channelId: string) {
    const page = await api.get<Message[]>(`/channels/${channelId}/messages?limit=${PAGE}`)
    this.messages = new Map(this.messages).set(channelId, page)
    if (page.length < PAGE) this.exhausted = new Set(this.exhausted).add(channelId)
  }
  async loadOlder(channelId: string) {
    const first = this.messages.get(channelId)?.[0]
    if (!first || this.loadingOlder.has(channelId) || this.exhausted.has(channelId)) return
    this.loadingOlder = new Set(this.loadingOlder).add(channelId)
    try {
      const page = await api.get<Message[]>(`/channels/${channelId}/messages?limit=${PAGE}&before=${first.id}`)
      this.messages = new Map(this.messages).set(channelId, [...page, ...(this.messages.get(channelId) || [])])
      if (page.length < PAGE) this.exhausted = new Set(this.exhausted).add(channelId)
    } finally {
      const s = new Set(this.loadingOlder); s.delete(channelId); this.loadingOlder = s
    }
  }
  /** A message by id, from cache or the server. Used for reply parents outside the loaded page. */
  async fetchMessage(id: string, channelId: string): Promise<Message | undefined> {
    const hit = this.messages.get(channelId)?.find((m) => m.id === id)
    if (hit) return hit
    try { return await api.get<Message>(`/messages/${id}`) } catch { return undefined }
  }

  async send(channelId: string, content: string, opts: { reply_to?: string; upload_ids?: string[] } = {}) {
    const m = await api.post<Message>(`/channels/${channelId}/messages`, { content, reply_to: opts.reply_to ?? null, upload_ids: opts.upload_ids ?? [] })
    this.upsert(m)
    this.markRead(channelId)
  }
  async edit(id: string, content: string) { this.upsert(await api.patch<Message>(`/messages/${id}`, { content })) }
  async remove(id: string, channelId: string) { await api.del(`/messages/${id}`); this.drop(id, channelId) }

  async react(m: Message, emoji: string) {
    const mine = m.reactions?.find((r) => r.emoji === emoji)?.user_ids.includes(this.me!.id)
    const reactions = mine
      ? await api.del<Reaction[]>(`/messages/${m.id}/reactions`, { emoji })
      : await api.put<Reaction[]>(`/messages/${m.id}/reactions`, { emoji })
    this.setReactions(m.channel_id, m.id, reactions)
  }
  private setReactions(channelId: string, id: string, reactions: Reaction[]) {
    const list = this.messages.get(channelId)
    const i = list?.findIndex((x) => x.id === id) ?? -1
    if (!list || i < 0) return
    this.messages = new Map(this.messages).set(channelId, list.with(i, { ...list[i]!, reactions }))
  }

  async openDm(userIds: string[]) {
    const c = await api.post<Channel>('/dms', { member_ids: [...new Set([...userIds, this.me!.id])] })
    if (!this.channels.some((x) => x.id === c.id)) this.channels = [...this.channels, c]
    if (!this.messages.has(c.id)) await this.loadLatest(c.id)
    return c
  }

  async search(q: string, channelId?: string): Promise<Message[]> {
    const p = new URLSearchParams({ q, limit: '50' })
    if (channelId) p.set('channel_id', channelId)
    return api.get<Message[]>(`/search/messages?${p}`)
  }

  private upsert(m: Message) {
    const list = this.messages.get(m.channel_id)
    if (!list) return // not loaded; read state carries the unread count
    const i = list.findIndex((x) => x.id === m.id)
    this.messages = new Map(this.messages).set(m.channel_id, i >= 0 ? list.with(i, m) : [...list, m])
  }
  private drop(id: string, channelId: string) {
    const list = this.messages.get(channelId)
    if (list) this.messages = new Map(this.messages).set(channelId, list.filter((x) => x.id !== id))
  }

  // --- realtime ---
  private connect() {
    if (this.ws) return
    const proto = location.protocol === 'https:' ? 'wss' : 'ws'
    const ws = new WebSocket(`${proto}://${location.host}/ws`)
    this.ws = ws
    ws.onopen = () => { this.connected = true; this.backoff = 800 }
    ws.binaryType = 'arraybuffer'
    ws.onmessage = (e) => this.handle(JSON.parse(typeof e.data === 'string' ? e.data : new TextDecoder().decode(e.data)) as Event)
    ws.onclose = () => {
      this.connected = false; this.ws = null
      if (!this.me) return
      setTimeout(() => this.connect(), this.backoff)
      this.backoff = Math.min(this.backoff * 2, 15_000)
    }
  }

  /** Tell the room you're typing. The server rate-limits to one per two seconds per channel. */
  sendTyping(channelId: string) {
    const now = Date.now()
    if (now - (this.lastTyping.get(channelId) || 0) < 2000 || this.ws?.readyState !== WebSocket.OPEN) return
    this.lastTyping.set(channelId, now)
    this.ws.send(JSON.stringify({ type: 'typing', channel_id: channelId }))
  }

  private async handle(ev: Event) {
    for (const fn of this.listeners) fn(ev)
    switch (ev.type) {
      case 'settings_updated': this.settings = ev.settings; break
      case 'object_presence': objects.presence = { ...objects.presence, [ev.id]: ev.user_ids }; break
      case 'resync': if (this.ready) await this.resync(); break
      case 'message_created':
      case 'message_edited': {
        const { type: _t, ...m } = ev
        if (!this.channels.some((c) => c.id === m.channel_id)) await this.resync()
        else this.upsert(m as Message)
        if (ev.type === 'message_created') { // they stopped typing
          const chan = this.typing.get(m.channel_id)
          if (chan?.has(m.author_id)) { const c = new Map(chan); c.delete(m.author_id); this.typing = new Map(this.typing).set(m.channel_id, c) }
        }
        break
      }
      case 'message_deleted':
        if (objects.active?.message_id === ev.id) { objects.active = null; objects.expanded = false }
        this.drop(ev.id, ev.channel_id); break
      case 'reactions_updated': this.setReactions(ev.channel_id, ev.message_id, ev.reactions); break
      case 'read_state_updated': this.setRead(ev.state); break
      case 'notification_preferences_updated': this.notif = ev.preferences; break
      case 'notification': this.alerts = [...this.alerts.slice(-20), ev]; break
      case 'call_state': call.receive(ev); if (!this.channel(ev.channel_id)) await this.resync(); break
      case 'presence': {
        const s = new Set(this.online); ev.online ? s.add(ev.user_id) : s.delete(ev.user_id); this.online = s
        break
      }
      case 'typing': {
        if (ev.user_id === this.me?.id) break
        const chan = new Map(this.typing.get(ev.channel_id) || [])
        chan.set(ev.user_id, Date.now() + 5000)
        this.typing = new Map(this.typing).set(ev.channel_id, chan)
        setTimeout(() => { this.typing = new Map(this.typing) }, 5100) // re-evaluate expiries
        break
      }
    }
  }

  typingNames(channelId: string): string[] {
    const now = Date.now()
    return [...(this.typing.get(channelId) || [])].filter(([, t]) => t > now).map(([id]) => this.name(id))
  }
}

export const store = new Store()
