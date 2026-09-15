import { upload, type Progress } from './upload'
import type { Upload } from './types'

export type PendingUpload = { id: number; file: File; progress: Progress; done?: Upload; error?: string; abort: AbortController }

// One queue per account/instance, owned by Store, independent of mounted composers.
export class Uploads {
  constructor(private origin: string) {}
  private seq = 0
  private channels = $state.raw<Record<string, PendingUpload[]>>({})
  forChannel(id: string) { return this.channels[id] || [] }
  active(id: string) { return this.forChannel(id).some(p => !p.done && !p.error) }
  private replace(id: string, items: PendingUpload[]) {
    const next = { ...this.channels }
    if (items.length) next[id] = items
    else delete next[id]
    this.channels = next
  }
  private patch(channel: string, id: number, part: Partial<PendingUpload>) {
    this.replace(channel, this.forChannel(channel).map(p => p.id === id ? { ...p, ...part } : p))
  }
  add(channel: string, files: File[]) {
    for (const file of files) {
      const p: PendingUpload = { id: ++this.seq, file, progress: { sent: 0, total: file.size }, abort: new AbortController() }
      if (file.size > 1024 ** 3) p.error = 'Over the 1 GB limit'
      this.replace(channel, [...this.forChannel(channel), p])
      if (p.error) continue
      void upload(channel, file, progress => this.patch(channel, p.id, { progress }), p.abort.signal, this.origin)
        .then(done => { this.patch(channel, p.id, { done }); window.dispatchEvent(new CustomEvent('den-sound-event', { detail: { origin: this.origin, sound: 'upload_complete' } })) })
        .catch(e => this.patch(channel, p.id, { error: e.name === 'AbortError' ? 'Cancelled' : e.message || 'Upload failed' }))
    }
  }
  remove(channel: string, item: PendingUpload) {
    item.abort.abort()
    this.replace(channel, this.forChannel(channel).filter(p => p.id !== item.id))
  }
  sent(channel: string, ids: string[]) {
    this.replace(channel, this.forChannel(channel).filter(p => !p.done || !ids.includes(p.done.id)))
  }
  clear() {
    for (const pending of Object.values(this.channels)) for (const p of pending) p.abort.abort()
    this.channels = {}
  }
}
