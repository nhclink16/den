// This entire module, React, tldraw, and its CSS load only when a canvas opens.
import { createElement } from 'react'
import { createRoot } from 'react-dom/client'
import { Tldraw, createTLStore, defaultShapeUtils, defaultBindingUtils, InstancePresenceRecordType, Box, atom, createTLUser,
  type Editor, type TLRecord, type TLPageId, type TLStore } from 'tldraw'
import 'tldraw/tldraw.css'
import './theme.css'
import { api, HttpError } from '../../lib/api'
import { store } from '../../lib/store.svelte'
import { objects } from '../../lib/objects.svelte'
import { upload } from '../../lib/upload'
import type { Event, LiveObject, ObjectSummary, ObjectVersion } from '../../lib/types'

const opens = new Map<string, number>()
const documentTypes = new Set(['document', 'page', 'shape', 'binding', 'asset'])
const isDocument = (r: unknown): r is TLRecord => !!r && typeof r === 'object' && documentTypes.has((r as TLRecord).typeName)
const hue = (id: string) => [...id].reduce((h, c) => (h * 31 + c.charCodeAt(0)) % 360, 7)
const color = (id: string) => `hsl(${hue(id)} 40% 65%)`
let cursorAt = 0
function open(id: string) {
  const n = opens.get(id) || 0; opens.set(id, n + 1)
  if (!n) store.sendEvent({ type: 'object_open', object_id: id })
}
function close(id: string) {
  const n = (opens.get(id) || 1) - 1
  if (n) opens.set(id, n)
  else { opens.delete(id); store.sendEvent({ type: 'object_close', object_id: id }) }
}

class Sync {
  tl: TLStore = createTLStore({ shapeUtils: defaultShapeUtils, bindingUtils: defaultBindingUtils })
  editor?: Editor
  version = 0
  pending = new Map<string, TLRecord | null>()
  inflight = new Map<string, TLRecord | null>()
  events: Extract<Event, { type: 'object_patched' }>[] = []
  loading = false
  dead = false
  sending = false
  timer?: ReturnType<typeof setTimeout>
  thumbTimer?: ReturnType<typeof setTimeout>
  cursorTimer?: ReturnType<typeof setTimeout>
  stopEvent: () => void
  stopStore?: () => void
  constructor(public object: ObjectSummary, public readonly: boolean, public report: (s: string) => void) {
    this.stopEvent = store.onEvent((ev) => {
      if (ev.type === 'resync') {
        store.sendEvent({ type: 'object_open', object_id: object.id })
        void this.reload().catch((e) => report(e.message))
      }
      if (ev.type === 'object_patched' && ev.id === object.id) {
        if (this.loading) this.events.push(ev)
        else this.receive(ev)
      }
      if (ev.type === 'object_cursor' && ev.id === object.id && ev.page_id.startsWith('page:')) {
        this.tl.mergeRemoteChanges(() => this.tl.put([InstancePresenceRecordType.create({
          id: InstancePresenceRecordType.createId(ev.user_id), userId: ev.user_id, userName: store.name(ev.user_id),
          currentPageId: ev.page_id as TLPageId, color: color(ev.user_id), lastActivityTimestamp: Date.now(),
          cursor: { x: ev.x, y: ev.y, type: 'default', rotation: 0 },
        })]))
      }
      if (ev.type === 'object_presence' && ev.id === object.id) {
        const gone = this.tl.allRecords().filter((r) => r.typeName === 'instance_presence' && !ev.user_ids.includes(r.userId))
        this.tl.mergeRemoteChanges(() => this.tl.remove(gone.map((r) => r.id)))
        this.thumbnail()
      }
    })
  }
  async start() {
    open(this.object.id)
    await this.reload()
    if (this.dead) return
    if (!this.readonly) this.stopStore = this.tl.listen(({ changes }) => {
      for (const r of Object.values(changes.added)) if (isDocument(r)) this.pending.set(r.id, r)
      for (const [, r] of Object.values(changes.updated)) if (isDocument(r)) this.pending.set(r.id, r)
      for (const r of Object.values(changes.removed)) if (isDocument(r)) this.pending.set(r.id, null)
      this.schedule(); this.thumbnail()
    }, { scope: 'document', source: 'user' })
  }
  async reload() {
    if (this.loading || this.dead) return
    this.loading = true
    try {
      let o = await api.get<LiveObject>(`/objects/${this.object.id}`)
      if (this.dead) return
      let records = Object.values(o.state).filter(isDocument)
      if (!records.length && !this.readonly) {
        records = [
          { id: 'document:document', typeName: 'document', gridSize: 10, name: '', meta: {} },
          { id: 'page:page', typeName: 'page', name: 'Page 1', index: 'a1', meta: {} },
        ] as TLRecord[]
        await api.post<ObjectVersion>(`/objects/${this.object.id}/patch`, { base_version: o.version, put: records, remove: [] })
        o = await api.get<LiveObject>(`/objects/${this.object.id}`)
        records = Object.values(o.state).filter(isDocument)
      }
      this.version = o.version
      // Keep unsent edits across gaps. Server records replace document state only.
      const merged = new Map(records.map((r) => [r.id as string, r]))
      for (const [id, r] of [...this.inflight, ...this.pending]) { if (r) merged.set(id, r); else merged.delete(id) }
      this.tl.mergeRemoteChanges(() => {
        const stale = this.tl.allRecords().filter((r) => isDocument(r) && !merged.has(r.id))
        this.tl.remove(stale.map((r) => r.id)); this.tl.put([...merged.values()])
      })
      this.report('')
    } finally {
      this.loading = false
      for (const ev of this.events.splice(0)) this.receive(ev)
    }
  }
  receive(ev: Extract<Event, { type: 'object_patched' }>) {
    if (this.dead || ev.version <= this.version) return
    if (ev.version !== this.version + 1) { void this.reload().catch((e) => this.report(e.message)); return }
    try {
      this.tl.mergeRemoteChanges(() => {
        this.tl.put(ev.put.filter(isDocument).filter((r) => !this.pending.has(r.id)))
        this.tl.remove(ev.remove.filter((id) => !this.pending.has(id) && isDocument(this.tl.get(id as TLRecord['id']))) as TLRecord['id'][])
      })
      this.version = ev.version; this.thumbnail()
    } catch { this.report('This canvas contains an invalid drawing record. Fix it with den canvas patch, then reopen it.') }
  }
  schedule(delay = 150) { clearTimeout(this.timer); this.timer = setTimeout(() => void this.flush(), delay) }
  async flush() {
    if (this.sending || !this.pending.size || this.loading || this.dead) return
    this.sending = true
    this.inflight = this.pending; this.pending = new Map()
    const body = { base_version: this.version, put: [...this.inflight.values()].filter((r) => r !== null), remove: [...this.inflight].filter(([, r]) => !r).map(([id]) => id) }
    try {
      await api.post(`/objects/${this.object.id}/patch`, body)
      // Version advances on the echoed event, never ahead of unreceived patches.
      this.report('')
    } catch (err) {
      this.pending = new Map([...this.inflight, ...this.pending])
      this.report((err as Error).message)
      if (err instanceof HttpError && err.status >= 400 && err.status < 500) return
    } finally {
      this.inflight = new Map(); this.sending = false
    }
    if (this.pending.size) this.schedule(800)
  }
  fit() { if (this.readonly && this.editor) { this.editor.updateViewportScreenBounds(this.editor.getContainer()); this.editor.zoomToFit({ animation: { duration: 0 } }) } }
  thumbnail() {
    this.fit()
    if (this.readonly || this.dead) return
    clearTimeout(this.thumbTimer)
    this.thumbTimer = setTimeout(() => void this.exportThumbnail(), 10_000)
  }
  async exportThumbnail() {
    const editor = this.editor
    if (!editor || this.dead || this.pending.size || this.sending || [...(objects.presence[this.object.id] || [])].sort()[0] !== store.me?.id) return
    try {
      const ids = [...editor.getCurrentPageShapeIds()]
      if (!ids.length) return
      const bounds = Box.Common(ids.map((id) => editor.getShapePageBounds(id)!).filter(Boolean)).expandBy(24)
      const { blob } = await editor.toImage(ids, { format: 'png', background: true, bounds, padding: 0, scale: 640 / bounds.w, pixelRatio: 1 })
      if (this.dead) return
      const u = await upload(this.object.channel_id, new File([blob], 'canvas.png', { type: 'image/png' }), () => {})
      if (!this.dead) await api.patch(`/objects/${this.object.id}`, { thumbnail_upload_id: u.id })
    } catch (err) { this.report(`Preview could not be saved: ${(err as Error).message}`) }
  }
  cursor() {
    if (this.readonly || this.cursorTimer || this.dead) return
    this.cursorTimer = setTimeout(() => {
      this.cursorTimer = undefined
      const editor = this.editor
      if (!editor || this.dead) return
      if (Date.now() - cursorAt < 50) { this.cursor(); return }
      cursorAt = Date.now()
      const { x, y } = editor.inputs.currentPagePoint
      store.sendEvent({ type: 'object_cursor', object_id: this.object.id, x, y, page_id: editor.getCurrentPageId() })
    }, 50)
  }
  async dispose() {
    this.stopStore?.()
    // Persist the final coalesced edits before releasing this view.
    clearTimeout(this.timer)
    while (this.sending) await new Promise((r) => setTimeout(r, 20))
    await this.flush()
    this.dead = true; this.stopEvent(); close(this.object.id)
    clearTimeout(this.timer); clearTimeout(this.thumbTimer); clearTimeout(this.cursorTimer)
  }
}

export async function mountCanvas(host: HTMLDivElement, object: ObjectSummary, readonly: boolean, report: (message: string) => void) {
  const sync = new Sync(object, readonly, report)
  try { await sync.start() } catch (err) { void sync.dispose(); throw err }
  const root = createRoot(host)
  const resize = new ResizeObserver(() => {
    requestAnimationFrame(() => {
      if (!sync.editor || sync.dead || !host.clientWidth || !host.clientHeight) return
      sync.editor.updateViewportScreenBounds(host)
      sync.editor.zoomToFit({ animation: { duration: 0 } })
    })
  }); resize.observe(host)
  const me = store.me!
  const user = createTLUser({ userPreferences: atom('den user', { id: me.id, name: store.name(me.id), color: color(me.id), colorScheme: 'dark' as const }) })
  root.render(createElement(Tldraw, {
    store: sync.tl, user, inferDarkMode: false, hideUi: readonly,
    onMount(editor: Editor) {
      sync.editor = editor
      editor.updateInstanceState({ isReadonly: readonly, isGridMode: true })
      if (editor.getCurrentPageShapeIds().size) editor.zoomToFit({ animation: { duration: 0 } })
      // Real editor handle on its DOM node, used by the browser acceptance script.
      Object.assign(host, { denEditor: editor })
      editor.on('event', (event) => { if (event.type === 'pointer' && event.name === 'pointer_move') sync.cursor() })
      sync.thumbnail()
    },
  }))
  return () => { resize.disconnect(); root.unmount(); void sync.dispose() }
}
