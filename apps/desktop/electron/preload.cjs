const { contextBridge, ipcRenderer } = require('electron')
const commands = new Set(['platform', 'session_get', 'session_set', 'session_clear', 'instances_get', 'instances_set', 'api_request', 'ptt_register', 'notify', 'badge', 'tray_state', 'deep_links', 'update_check', 'update_restart', 'activity_current', 'share_picker_choose', 'share_picker_sources'])
const events = new Set(['ptt', 'tray-action', 'notification-open', 'deep-link://new-url', 'window-background', 'activity', 'share-picker'])
contextBridge.exposeInMainWorld('denDesktop', {
  invoke(command, args) {
    if (!commands.has(command)) return Promise.reject(Error('Unknown desktop command'))
    return ipcRenderer.invoke('den:command', command, args)
  },
  async listen(event, handler) {
    if (!events.has(event)) throw Error('Unknown desktop event')
    const listener = (_event, payload) => handler(payload)
    ipcRenderer.on(`den:${event}`, listener)
    return () => ipcRenderer.removeListener(`den:${event}`, listener)
  },
})
