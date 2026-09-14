// Native credentials live in the OS keychain. This bridge never caches tokens in web storage.
type Bridge = {
  core: { invoke<T>(command: string, args?: Record<string, unknown>): Promise<T>; convertFileSrc(path: string, protocol?: string): string }
  event: { listen<T>(event: string, handler: (event: { payload: T }) => void): Promise<() => void> }
}
declare global { interface Window { __TAURI__?: Bridge } }
export const native = !!window.__TAURI__
export const invoke = <T>(command: string, args?: Record<string, unknown>) => window.__TAURI__!.core.invoke<T>(command, args)
export const listen = <T>(event: string, handler: (payload: T) => void) => window.__TAURI__!.event.listen<T>(event, e => handler(e.payload))
export function originOf(input: string) {
  const u = new URL(input.includes('://') ? input : `https://${input}`)
  if (u.username || u.password || !(u.protocol === 'https:' || (u.protocol === 'http:' && ['localhost', '127.0.0.1', '[::1]'].includes(u.hostname)))) throw Error('Use an HTTPS server URL, or localhost for development.')
  return u.origin
}
let origin = native ? localStorage.getItem('den.native.origin') || 'https://denchat.app' : location.origin
export function activeOrigin() { return origin }
export function setOrigin(value: string) { origin = value; if (native) localStorage.setItem('den.native.origin', value) }
export function mediaUrl(path: string, server = origin) {
  if (!native || !path.startsWith('/uploads/')) return path
  return `${window.__TAURI__!.core.convertFileSrc('', 'den-media')}${path.replace(/^\//, '')}?origin=${encodeURIComponent(server)}`
}
