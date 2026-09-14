// Track creation receipts, never delete a room-wide before/after difference.
import assert from 'node:assert/strict'
export const keepSmoke = process.env.DEN_SMOKE_KEEP === '1'
const originalFetch = globalThis.fetch
const active = new Set()
const creates = path => /^\/channels\/[^/]+\/(messages|objects)$|^\/hosts\/[^/]+\/(sessions|requests)$|^\/sessions\/[^/]+\/share$|^\/uploads$|^\/requests\/[^/]+\/decide$/.test(path)
globalThis.fetch = async (...args) => {
  const response = await originalFetch(...args)
  const method = args[1]?.method || args[0]?.method || 'GET'
  const url = new URL(typeof args[0] === 'string' || args[0] instanceof URL ? args[0] : args[0].url)
  if (method === 'POST' && response.ok && creates(url.pathname)) {
    for (const cleanup of active) if (url.origin === cleanup.base) cleanup.record(url.pathname, await response.clone().json())
  }
  return response
}
export class SmokeCleanup {
  messages = new Set(); objects = new Set(); sessions = new Set(); uploads = new Set(); grants = new Set()
  pending = []; errors = []; restores = []
  constructor(base, token) { this.base = new URL(base).origin; this.token = token }
  async api(method, path, body, missing = false) {
    const r = await originalFetch(this.base + path, { method, headers: { authorization: `Bearer ${this.token}`, 'content-type': 'application/json' }, body: body === undefined ? undefined : JSON.stringify(body) })
    if (missing && r.status === 404) return null
    assert(r.ok, `cleanup ${method} ${path}: ${r.status}`)
    return r.status === 204 ? null : r.json()
  }
  static async start(base, token) {
    const c = new SmokeCleanup(base, token)
    c.before = new Set((await c.snapshot()).map(m => m.id)); active.add(c); return c
  }
  static async login(base, username, password) {
    const r = await originalFetch(base+'/auth/login', { method:'POST', headers:{'content-type':'application/json'}, body:JSON.stringify({username,password}) })
    assert(r.ok, `cleanup login: ${r.status}`)
    const session = await r.json(), c = await SmokeCleanup.start(base, session.token)
    c.logout = true; return c
  }
  async snapshot() {
    const messages = []
    for (const channel of await this.api('GET', '/channels')) {
      let before = ''
      for (;;) {
        const page = await this.api('GET', `/channels/${channel.id}/messages?limit=200${before ? `&before=${before}` : ''}`)
        messages.push(...page)
        if (page.length < 200) break
        before = page[0].id
      }
    }
    return messages
  }
  record(path, value) {
    if (/\/messages$/.test(path)) this.messages.add(value.id)
    if (value.message_id && value.kind) {
      this.messages.add(value.message_id); this.objects.add(value.id)
      if (/\/hosts\/[^/]+\/sessions$/.test(path)) this.sessions.add(value.id)
    }
    if (path === '/uploads') this.uploads.add(value.id)
    if (value.grant_id) this.grants.add(value.grant_id)
  }
  watch(context) {
    context.on('response', response => {
      const path = new URL(response.url()).pathname
      if (new URL(response.url()).origin !== this.base || response.request().method() !== 'POST' || !response.ok() || !creates(path)) return
      this.pending.push(response.json().then(value => this.record(path, value)).catch(e => this.errors.push(e)))
    })
  }
  async finish(browser) {
    active.delete(this)
    // Read creation responses before closing pages, then stop all thumbnail writers.
    await Promise.all(this.pending)
    try { await browser?.close() } catch(e) { this.errors.push(e) }
    await Promise.all(this.pending)
    const failures = [...this.errors]
    const attempt = async fn => { try { return await fn() } catch(e) { failures.push(e) } }
    if (!keepSmoke) {
      for (const id of this.sessions) await attempt(async () => {
        await this.api('DELETE', `/sessions/${id}`, undefined, true)
        const o = await this.api('GET', `/objects/${id}`, undefined, true)
        if (o?.state.terminal.recording_upload_id) this.uploads.add(o.state.terminal.recording_upload_id)
      })
      for (const id of this.objects) await attempt(async () => {
        const o = await this.api('GET', `/objects/${id}`, undefined, true)
        if (o?.thumbnail_upload_id) this.uploads.add(o.thumbnail_upload_id)
      })
      for (const id of [...this.messages].reverse()) await attempt(async () => {
        const message = await this.api('GET', `/messages/${id}`, undefined, true)
        for (const upload of message?.attachments || []) this.uploads.add(upload.id)
        await this.api('DELETE', `/messages/${id}`, undefined, true)
      })
      for (const id of this.grants) await attempt(() => this.api('DELETE', `/grants/${id}`, undefined, true))
      for (const id of this.uploads) await attempt(() => this.api('DELETE', `/uploads/${id}`, undefined, true))
      for (const restore of this.restores.reverse()) await attempt(restore)
      await attempt(async () => {
        const added = (await this.snapshot()).filter(m => !this.before.has(m.id))
        assert.equal(added.length, 0, `smoke left ${added.length} new messages/objects/requests`)
        for (const id of this.objects) assert.equal(await this.api('GET', `/objects/${id}`, undefined, true), null)
      })
      if (!failures.length) console.log(`Cleanup verified: ${this.messages.size} messages, ${this.objects.size} objects/requests, ${this.uploads.size} uploads; no new room artifacts.`)
    } else console.log('DEN_SMOKE_KEEP=1: keeping smoke artifacts for inspection.')
    if (this.logout) await attempt(() => this.api('POST', '/auth/logout', {}))
    if (failures.length) throw new AggregateError(failures, 'Smoke cleanup failed')
  }
}
