import { isDesktop, type DesktopCommand, type DesktopEvent } from './desktop'
// Native credentials live in the OS keychain. This bridge never caches tokens in web storage.
type Bridge = {
  core: { invoke<T>(command: string, args?: Record<string, unknown>): Promise<T>; convertFileSrc(path: string, protocol?: string): string }
  event: { listen<T>(event: string, handler: (event: { payload: T }) => void): Promise<() => void> }
}
declare global { interface Window { __TAURI__?: Bridge } }
export const native = isDesktop()
export const invoke = <T>(command: DesktopCommand, args?: Record<string, unknown>) => window.denDesktop ? window.denDesktop.invoke<T>(command, args) : window.__TAURI__!.core.invoke<T>(command, args)
export const listen = <T>(event: DesktopEvent, handler: (payload: T) => void) => window.denDesktop ? window.denDesktop.listen<T>(event, handler) : window.__TAURI__!.event.listen<T>(event, e => handler(e.payload))
export function originOf(input: string) {
  const u = new URL(input.includes('://') ? input : `https://${input}`)
  if (u.username || u.password || !(u.protocol === 'https:' || (u.protocol === 'http:' && ['localhost', '127.0.0.1', '[::1]'].includes(u.hostname)))) throw Error('Use an HTTPS server URL, or localhost for development.')
  return u.origin
}
let origin = native ? localStorage.getItem('den.native.origin') || 'https://denchat.app' : location.origin
export function activeOrigin() { return origin }
export function setOrigin(value: string) { origin = value; if (native) localStorage.setItem('den.native.origin', value) }
// Account images the desktop fetches with the stored session: attachments, profile
// pictures and banners, and wallpapers. Anything else stays a plain path.
const MEDIA = /^\/(uploads\/[^/]+\/(file|thumbnail)|users\/[^/]+\/(avatar|banner)|users\/me\/background\/image|users\/me\/backgrounds\/[0-9a-f]{64}(\/preview)?)(\?|$)/
export function mediaUrl(path: string, server = origin) {
  if (!native || !MEDIA.test(path)) return path
  const [bare, query] = path.split('?')
  // Keep ?v= so a replaced picture gets a new URL, and the browser a new fetch.
  return `${window.denDesktop ? 'den-media://app/' : window.__TAURI__!.core.convertFileSrc('', 'den-media')}${bare!.replace(/^\//, '')}?origin=${encodeURIComponent(server)}${query ? `&${query}` : ''}`
}
