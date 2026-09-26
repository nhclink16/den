// Only the bundled desktop renderer receives this bridge. The web app keeps cookie auth.
export type DesktopCommand = 'platform' | 'session_get' | 'session_set' | 'session_clear'
  | 'instances_get' | 'instances_set' | 'api_request' | 'ptt_register' | 'notify'
  | 'badge' | 'tray_state' | 'deep_links' | 'update_check' | 'update_restart' | 'activity_current'
  | 'share_picker_choose' | 'share_picker_sources'
export type DesktopEvent = 'ptt' | 'tray-action' | 'notification-open'
  | 'deep-link://new-url' | 'window-background' | 'activity' | 'share-picker'
export interface DesktopBridge {
  invoke<T>(command: DesktopCommand, args?: Record<string, unknown>): Promise<T>
  listen<T>(event: DesktopEvent, handler: (payload: T) => void): Promise<() => void>
}
declare global { interface Window { denDesktop?: DesktopBridge } }
export const isDesktop = () => !!(window.denDesktop || window.__TAURI__)
