// Chunked, resumable upload against /uploads. 8 MiB chunks, server tells us the offset.
import { api } from './api'
import type { Upload } from './types'

const CHUNK = 8 * 1024 * 1024

export type Progress = { sent: number; total: number }

export async function upload(channelId: string, file: File, onProgress: (p: Progress) => void, signal?: AbortSignal): Promise<Upload> {
  let u = await api.post<Upload>('/uploads', {
    channel_id: channelId,
    filename: file.name,
    content_type: file.type || 'application/octet-stream',
    size: file.size,
  })
  let offset = u.offset
  while (offset < file.size) {
    if (signal?.aborted) throw new DOMException('Upload cancelled', 'AbortError')
    const end = Math.min(offset + CHUNK, file.size)
    try {
      u = await api.patchRaw<Upload>(`/uploads/${u.id}`, file.slice(offset, end), { 'upload-offset': String(offset) })
      offset = u.offset
    } catch (e) {
      // Offset mismatch or a dropped request: ask the server where it is and continue.
      u = await api.get<Upload>(`/uploads/${u.id}`)
      offset = u.offset
      if (offset >= file.size) break
      if ((e as { status?: number }).status !== 409) await new Promise((r) => setTimeout(r, 800))
    }
    onProgress({ sent: offset, total: file.size })
  }
  return api.post<Upload>(`/uploads/${u.id}/complete`)
}

export const fileUrl = (id: string) => `/uploads/${id}/file`
