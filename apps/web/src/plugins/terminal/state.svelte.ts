import { api } from '../../lib/api'
import { store } from '../../lib/store.svelte'
import type { Host, Grant, TerminalState, AccessRequest, LiveObject, ObjectSummary } from '../../lib/types'
export const terminals = $state({ hosts: [] as Host[], grants: [] as Grant[], sessions: {} as Record<string, TerminalState>, requests: {} as Record<string, AccessRequest>, previews: {} as Record<string, string> })
export async function catalog() {
  if (!store.me) return
  const origin = store.origin, request = store.api
  try { const data = await Promise.all([request.get<Host[]>('/hosts'), request.get<Grant[]>('/grants')]); if (store.origin === origin) [terminals.hosts, terminals.grants] = data } catch { /* A reconnect retries. */ }
}
export async function load(object: ObjectSummary) {
  const o = await api.get<LiveObject>(`/objects/${object.id}`)
  if (o.kind === 'terminal') { const t = o.state.terminal as TerminalState; terminals.sessions[t.id] = t; return t }
  const r = o.state.request as AccessRequest; terminals.requests[r.id] = r
}
export function capability(t: TerminalState, control = false) {
  return t.owner_id === store.me?.id || terminals.grants.some(g => g.host_id === t.host_id && g.grantee_id === store.me?.id && !g.revoked_at && (!g.expires_at || g.expires_at * 1000 > Date.now()) && (!control || g.capability === 'terminal_control'))
}
store.onEvent(ev => {
  if (ev.type === 'resync') void catalog()
  if (ev.type === 'terminal_state') terminals.sessions[ev.session.id] = ev.session
  if (ev.type === 'access_decided') {
    terminals.requests[ev.request.id] = ev.request; void catalog()
    if (ev.request.status === 'denied' && ev.request.requester_id === store.me?.id) store.toast = `${store.user(ev.request.owner_id)?.username || 'The owner'} said not now.`
  }
})
setInterval(() => { if (store.me) void catalog() }, 5000)

window.addEventListener('den-instance', () => {
  terminals.hosts = []; terminals.grants = []; terminals.sessions = {}; terminals.requests = {}; terminals.previews = {}
  void catalog()
})
