// URL shapes, kept out of the router so they can be pinned directly.
//
// Canonical saved routes:
//   /c/:channel                     a room
//   /c/:channel?m=:message          a room, revealing one message
//   /c/:channel/t/:thread           a conversation
//   /c/:channel/t/:thread?m=:message
//   /c/:channel?reply=:root         a conversation the server has no thread for
//                                   yet, identified by the root being replied to
export type Route =
  | { name: 'login' }
  | { name: 'channel'; id: string; thread?: string; message?: string; reply?: string }
  | { name: 'inbox' }
  | { name: 'settings'; section?: string }
  | { name: 'search'; q: string; channel?: string }
  | { name: 'home' }

export function parse(path: string, search = ''): Route {
  const p = path.split('?')[0]!.replace(/\/+$/, '') || '/'
  const query = new URLSearchParams(search)
  if (p === '/login') return { name: 'login' }
  if (p === '/inbox') return { name: 'inbox' }
  if (p.startsWith('/settings')) return { name: 'settings', section: p.split('/')[2] }
  if (p === '/find') return { name: 'search', q: query.get('q') || '', channel: query.get('in') || undefined }
  const c = p.match(/^\/c\/([A-Z0-9]+)(?:\/t\/([A-Z0-9]+))?$/)
  if (c) {
    return {
      name: 'channel', id: c[1]!,
      thread: c[2] || undefined,
      message: query.get('m') || undefined,
      reply: query.get('reply') || undefined,
    }
  }
  return { name: 'home' }
}
