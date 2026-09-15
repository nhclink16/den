import { api } from '../../lib/api'
import { store } from '../../lib/store.svelte'
import type { DirectToken, TerminalFrame } from '../../lib/types'
type Sink = { resize: (cols: number, rows: number) => void; bytes: (bytes: Uint8Array) => void; reset: () => void; path: (direct: boolean) => void }
type Stream = { sinks: Set<Sink>; chunks: Uint8Array[]; size: number; direct?: WebSocket; mode: 'pending' | 'relay' | 'direct'; disposed: boolean; timer?: ReturnType<typeof setTimeout>; input: number[]; flush?: ReturnType<typeof setTimeout> }
const streams = new Map<string, Stream>()
function reset(s: Stream) { s.chunks = []; s.size = 0; for (const sink of s.sinks) sink.reset() }
function output(s: Stream, bytes: Uint8Array) {
  s.chunks.push(bytes); s.size += bytes.length
  while (s.size > 256 * 1024 && s.chunks.length > 1) s.size -= s.chunks.shift()!.length
  for (const sink of s.sinks) sink.bytes(bytes)
}
function relay(id: string, s: Stream) {
  if (s.disposed) return
  s.mode = 'relay'; for (const sink of s.sinks) sink.path(false)
  store.sendTerminal({ type: 'terminal_open', session_id: id })
}
async function connect(id: string, s: Stream) {
  s.mode = 'pending'
  try {
    const ticket = await api.post<DirectToken>(`/sessions/${id}/direct-token`, {})
    if (s.disposed) return
    if (!ticket.url) { relay(id, s); return }
    const socket = new WebSocket(ticket.url); socket.binaryType = 'arraybuffer'; s.direct = socket
    let started = false
    const timeout = setTimeout(() => { if (!started) {socket.close(); relay(id, s)} }, 700)
    socket.onopen = () => socket.send(new TextEncoder().encode(JSON.stringify({ token: ticket.token, session_id: id, input: false })))
    socket.onmessage = ev => {
      if (s.disposed) return
      const frame = JSON.parse(new TextDecoder().decode(ev.data))
      if (!started) {
        started = true; clearTimeout(timeout); s.mode = 'direct'; reset(s)
        for (const sink of s.sinks) sink.path(true)
        store.sendTerminal({ type: 'terminal_open', session_id: id })
      }
      if (frame.type === 'resize') for (const sink of s.sinks) sink.resize(frame.cols, frame.rows)
      if (frame.type === 'output') output(s, new Uint8Array(frame.bytes))
    }
    socket.onerror = () => { /* onclose switches to the relay */ }
    socket.onclose = () => { clearTimeout(timeout); if (!s.disposed && s.direct === socket) {s.direct = undefined; relay(id, s)} }
    s.timer = setTimeout(() => { socket.onclose = null; socket.close(); void connect(id, s) }, 50_000)
  } catch { relay(id, s) }
}
store.onEvent(ev => {
  if (ev.type === 'terminal_output') {
    const s = streams.get(ev.session_id)
    if (s?.mode === 'relay') {if (ev.connection_id) reset(s); output(s, new Uint8Array(ev.bytes))}
  }
  if (ev.type === 'resync') for (const [id, s] of streams) { if (s.mode !== 'direct') relay(id, s) }
})
export function subscribe(id: string, sink: Sink) {
  let s = streams.get(id)
  if (!s) { s = { sinks: new Set(), chunks: [], size: 0, mode: 'pending', disposed: false, input: [] }; streams.set(id, s); void connect(id, s) }
  const stream = s; stream.sinks.add(sink); sink.path(stream.mode === 'direct')
  for (const chunk of stream.chunks) sink.bytes(chunk)
  if (stream.sinks.size > 1) store.sendTerminal({ type: 'terminal_open', session_id: id })
  const send = (frame: TerminalFrame) => { if (stream.direct?.readyState === WebSocket.OPEN && stream.mode === 'direct') stream.direct.send(new TextEncoder().encode(JSON.stringify(frame))); else store.sendTerminal(frame) }
  return {
    input(text: string | Uint8Array) {
      stream.input.push(...(typeof text === 'string' ? new TextEncoder().encode(text) : text))
      if (!stream.flush) stream.flush = setTimeout(() => { const bytes = stream.input.splice(0); stream.flush = undefined; for (let i = 0; i < bytes.length; i += 8192) send({ type: 'terminal_input', session_id: id, bytes: bytes.slice(i, i + 8192) }) }, 16)
    },
    resize(cols: number, rows: number) { store.sendTerminal({ type: 'terminal_resize', session_id: id, cols, rows }) },
    destroy() { stream.sinks.delete(sink); if (stream.sinks.size) return; stream.disposed = true; clearTimeout(stream.timer); clearTimeout(stream.flush); stream.direct?.close(); store.sendTerminal({ type: 'terminal_close', session_id: id }); streams.delete(id) },
  }
}
