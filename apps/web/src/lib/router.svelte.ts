// Tiny history router. Routes: /login, /c/:id, /inbox, /settings, /
export type Route =
  | { name: 'login' }
  | { name: 'channel'; id: string }
  | { name: 'inbox' }
  | { name: 'settings'; section?: string }
  | { name: 'home' }

function parse(path: string): Route {
  const p = path.replace(/\/+$/, '') || '/'
  if (p === '/login') return { name: 'login' }
  if (p === '/inbox') return { name: 'inbox' }
  if (p.startsWith('/settings')) return { name: 'settings', section: p.split('/')[2] }
  const c = p.match(/^\/c\/([A-Z0-9]+)$/)
  if (c) return { name: 'channel', id: c[1]! }
  return { name: 'home' }
}

let route = $state<Route>(parse(location.pathname))

export const router = {
  get route() { return route },
  go(path: string, replace = false) {
    if (location.pathname === path) return
    history[replace ? 'replaceState' : 'pushState']({}, '', path)
    route = parse(path)
  },
}

window.addEventListener('popstate', () => { route = parse(location.pathname) })
