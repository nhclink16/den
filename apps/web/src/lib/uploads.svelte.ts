import { upload, type Progress } from './upload'
import { conversationKey, type Conversation } from './conversation'
import { append, busy, busyInChannel, consume, drop, patch, tray, type Queues } from './upload-queue'
import type { Upload } from './types'

export type PendingUpload = { id: number; file: File; progress: Progress; done?: Upload; error?: string; abort: AbortController }

// One queue per account/instance, owned by Store, independent of mounted
// composers. The reactive owner only; the tray transforms live in upload-queue.
//
// The TRAY is keyed by conversation, so a file staged in the room cannot appear
// in, or block sending from, a conversation inside the same channel. The SERVER
// upload still belongs to its channel, which is why the channel is passed
// separately rather than parsed back out of the key.
export class Uploads {
  constructor(private origin: string) {}
  private seq = 0
  private queues = $state.raw<Queues<PendingUpload>>({})
  forConversation(c: Conversation) { return tray(this.queues, conversationKey(c)) }
  active(c: Conversation) { return busy(this.forConversation(c)) }
  activeInChannel(channelId: string) { return busyInChannel(this.queues, channelId) }
  add(c: Conversation, files: File[]) {
    const key = conversationKey(c)
    for (const file of files) {
      const p: PendingUpload = { id: ++this.seq, file, progress: { sent: 0, total: file.size }, abort: new AbortController() }
      if (file.size > 1024 ** 3) p.error = 'Over the 1 GB limit'
      this.queues = append(this.queues, key, p)
      if (p.error) continue
      const at = (part: Partial<PendingUpload>) => { this.queues = patch(this.queues, key, p.id, part) }
      // The file is uploaded to the CHANNEL; only the tray is per conversation.
      void upload(c.channelId, file, progress => at({ progress }), p.abort.signal, this.origin)
        .then(done => { at({ done }); window.dispatchEvent(new CustomEvent('den-sound-event', { detail: { origin: this.origin, sound: 'upload_complete' } })) })
        .catch(e => at({ error: e.name === 'AbortError' ? 'Cancelled' : e.message || 'Upload failed' }))
    }
  }
  remove(c: Conversation, item: PendingUpload) {
    item.abort.abort()
    this.queues = drop(this.queues, conversationKey(c), item.id)
  }
  /// Consume exactly the IDs that were transmitted, in the conversation they were
  /// submitted from, even if a newer draft has since been staged there.
  sent(c: Conversation, ids: string[]) {
    this.queues = consume(this.queues, conversationKey(c), ids)
  }
  clear() {
    for (const pending of Object.values(this.queues)) for (const p of pending) p.abort.abort()
    this.queues = {}
  }
}
