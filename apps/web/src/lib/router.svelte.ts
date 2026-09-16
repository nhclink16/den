// Tiny history router. Routes: /login, /c/:id, /inbox, /find, /settings, /spotify/callback, /
export type Route =
  | { name: 'login' }
  | { name: 'channel'; id: string }
  | { name: 'inbox' }
  | { name: 'settings'; section?: string }
  // Spotify redirects the browser back here. `spotify` is not an API segment on the
  // server, so this path falls through to index.html and stays a client route.
  | { name: 'spotify-callback'; code: string; state: string; error: string }
  | { name: 'search'; q: string; channel?: string }
  | { name: 'home' }

function parse(path: string): Route {
  const p = path.split('?')[0]!.replace(/\/+$/, '') || '/'
  if (p === '/login') return { name: 'login' }
  if (p === '/inbox') return { name: 'inbox' }
  if (p.startsWith('/settings')) return { name: 'settings', section: p.split('/')[2] }
  if (p === '/spotify/callback') {
    const s = new URLSearchParams(location.search)
    return { name: 'spotify-callback', code: s.get('code') || '', state: s.get('state') || '', error: s.get('error') || '' }
  }
  if (p === '/find') { const s = new URLSearchParams(location.search); return { name: 'search', q: s.get('q') || '', channel: s.get('in') || undefined } }
  const c = p.match(/^\/c\/([A-Z0-9]+)$/)
  if (c) return { name: 'channel', id: c[1]! }
  return { name: 'home' }
}

let route = $state<Route>(parse(location.pathname))

export const router = {
  get route() { return route },
  go(path: string, replace = false) {
    if (location.pathname + location.search === path) return
    history[replace ? 'replaceState' : 'pushState']({}, '', path)
    route = parse(path)
  },
}

window.addEventListener('popstate', () => { route = parse(location.pathname) })
