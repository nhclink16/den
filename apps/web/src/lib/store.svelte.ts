// All client state in one place, Svelte 5 runes. The server is the truth; this is a cache
// that the WebSocket keeps warm and a resync throws away.
import { api, setCsrf } from './api'
import type { Category, Channel, Event, Message, Session, User } from './types'
import { mentions } from './markdown'

const LAST_READ_KEY = 'den.lastRead'
const PREFS_KEY = 'den.prefs'

export type Prefs = {
  sidebar: boolean
  members: boolean
  subscribed: string[] // channel ids that notify on every message
  sounds: boolean
}

function loadJson<T>(key: string, fallback: T): T {
  try { return { ...fallback, ...JSON.parse(localStorage.getItem(key) || '{}') } } catch { return fallback }
}

class Store {
  me = $state<User | null>(null)
  users = $state<Map<string, User>>(new Map())
  channels = $state<Channel[]>([])
  categories = $state<Category[]>([])
  messages = $state<Map<string, Message[]>>(new Map())
  online = $state<Set<string>>(new Set())
  typing = $state<Map<string, Map<string, number>>>(new Map()) // channel -> user -> expiry
  lastRead = $state<Record<string, string>>(loadJson(LAST_READ_KEY, {}))
  prefs = $state<Prefs>(loadJson(PREFS_KEY, { sidebar: true, members: true, subscribed: [], sounds: false }))
  connected = $state(false)
  ready = $state(false)
  loadingOlder = $state<Set<string>>(new Set())
  exhausted = $state<Set<string>>(new Set())
  private ws: WebSocket | null = null
  private backoff = 800

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

  // Unread: last message id vs the last id we marked read. IDs are ULIDs, so string compare works.
  unread(channelId: string): { count: number; mention: boolean } {
    const msgs = this.messages.get(channelId) || []
    const last = this.lastRead[channelId] || ''
    let count = 0, mention = false
    for (let i = msgs.length - 1; i >= 0; i--) {
      const m = msgs[i]!
      if (m.id <= last) break
      if (m.author_id === this.me?.id) continue
      count++
      if (this.me && mentions(m.content, this.users).includes(this.me.id)) mention = true
    }
    return { count, mention }
  }

  markRead(channelId: string) {
    const msgs = this.messages.get(channelId)
    const last = msgs?.at(-1)?.id
    if (!last || this.lastRead[channelId] === last) return
    this.lastRead = { ...this.lastRead, [channelId]: last }
    localStorage.setItem(LAST_READ_KEY, JSON.stringify(this.lastRead))
  }

  savePrefs(patch: Partial<Prefs>) {
    this.prefs = { ...this.prefs, ...patch }
    localStorage.setItem(PREFS_KEY, JSON.stringify(this.prefs))
  }

  // --- session ---
  async login(username: string, password: string) {
    const s = await api.post<Session>('/auth/login', { username, password })
    setCsrf(s.csrf_token)
    this.me = s.user
    await this.boot()
  }

  async register(username: string, password: string, invite: string) {
    const s = await api.post<Session>('/auth/register', { username, password, invite })
    setCsrf(s.csrf_token)
    this.me = s.user
    await this.boot()
  }

  async logout() {
    try { await api.post('/auth/logout') } catch { /* already gone */ }
    setCsrf(null)
    this.ws?.close()
    this.me = null
    this.ready = false
  }

  async resume(): Promise<boolean> {
    try {
      this.me = await api.get<User>('/users/me')
      await this.boot()
      return true
    } catch {
      return false
    }
  }

  private async boot() {
    await this.resync()
    this.ready = true
    this.connect()
  }

  async resync() {
    const [users, channels, categories] = await Promise.all([
      api.get<User[]>('/users'),
      api.get<Channel[]>('/channels'),
      api.get<Category[]>('/categories'),
    ])
    this.users = new Map(users.map((u) => [u.id, u]))
    this.channels = channels
    this.categories = categories.sort((a, b) => a.position - b.position)
    // Refresh the tail of every channel we already had, so unread counts survive a reconnect.
    await Promise.all(channels.map((c) => this.loadLatest(c.id)))
  }

  // --- messages ---
  async loadLatest(channelId: string) {
    const page = await api.get<Message[]>(`/channels/${channelId}/messages?limit=50`)
    this.messages = new Map(this.messages).set(channelId, page)
    if (page.length < 50) this.exhausted = new Set(this.exhausted).add(channelId)
  }

  async loadOlder(channelId: string) {
    const cur = this.messages.get(channelId) || []
    const first = cur[0]
    if (!first || this.loadingOlder.has(channelId) || this.exhausted.has(channelId)) return
    this.loadingOlder = new Set(this.loadingOlder).add(channelId)
    try {
      const page = await api.get<Message[]>(`/channels/${channelId}/messages?limit=50&before=${first.id}`)
      this.messages = new Map(this.messages).set(channelId, [...page, ...cur])
      if (page.length < 50) this.exhausted = new Set(this.exhausted).add(channelId)
    } finally {
      const s = new Set(this.loadingOlder); s.delete(channelId); this.loadingOlder = s
    }
  }

  async send(channelId: string, content: string, opts: { reply_to?: string; upload_ids?: string[] } = {}) {
    const m = await api.post<Message>(`/channels/${channelId}/messages`, { content, reply_to: opts.reply_to ?? null, upload_ids: opts.upload_ids ?? [] })
    this.upsert(m)
    this.markRead(channelId)
  }

  async edit(id: string, content: string) {
    this.upsert(await api.patch<Message>(`/messages/${id}`, { content }))
  }

  async remove(id: string, channelId: string) {
    await api.del(`/messages/${id}`)
    this.drop(id, channelId)
  }

  async openDm(userIds: string[]) {
    const c = await api.post<Channel>('/dms', { member_ids: [...new Set([...userIds, this.me!.id])] })
    if (!this.channels.some((x) => x.id === c.id)) this.channels = [...this.channels, c]
    if (!this.messages.has(c.id)) await this.loadLatest(c.id)
    return c
  }

  private upsert(m: Message) {
    const list = this.messages.get(m.channel_id) || []
    const i = list.findIndex((x) => x.id === m.id)
    const next = i >= 0 ? list.with(i, m) : [...list, m]
    this.messages = new Map(this.messages).set(m.channel_id, next)
  }

  private drop(id: string, channelId: string) {
    const list = this.messages.get(channelId)
    if (!list) return
    this.messages = new Map(this.messages).set(channelId, list.filter((x) => x.id !== id))
  }

  // --- realtime ---
  private connect() {
    if (this.ws) return
    const proto = location.protocol === 'https:' ? 'wss' : 'ws'
    const ws = new WebSocket(`${proto}://${location.host}/ws`)
    this.ws = ws
    ws.onopen = () => {
      this.connected = true; this.backoff = 800
      // Until the server broadcasts presence, at least light up yourself.
      if (this.me) this.online = new Set(this.online).add(this.me.id)
    }
    ws.onmessage = (e) => this.handle(JSON.parse(e.data) as Event)
    ws.onclose = () => {
      this.connected = false
      this.ws = null
      if (!this.me) return
      setTimeout(() => this.connect(), this.backoff)
      this.backoff = Math.min(this.backoff * 2, 15_000)
    }
  }

  private async handle(ev: Event) {
    switch (ev.type) {
      case 'resync': {
        if (this.ready) await this.resync()
        break
      }
      case 'message_created':
      case 'message_edited': {
        const { type: _t, ...m } = ev
        if (!this.channels.some((c) => c.id === m.channel_id)) await this.resync()
        else this.upsert(m as Message)
        break
      }
      case 'message_deleted': this.drop(ev.id, ev.channel_id); break
      case 'presence': {
        const s = new Set(this.online)
        ev.online ? s.add(ev.user_id) : s.delete(ev.user_id)
        this.online = s
        break
      }
      case 'typing': {
        const chan = new Map(this.typing.get(ev.channel_id) || [])
        chan.set(ev.user_id, Date.now() + 6000)
        this.typing = new Map(this.typing).set(ev.channel_id, chan)
        break
      }
    }
  }

  typingNames(channelId: string): string[] {
    const now = Date.now()
    return [...(this.typing.get(channelId) || [])].filter(([id, t]) => t > now && id !== this.me?.id).map(([id]) => this.name(id))
  }
}

export const store = new Store()
