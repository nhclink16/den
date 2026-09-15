const { app, Tray, Menu, Notification, nativeImage, systemPreferences, powerMonitor } = require('electron')
const path = require('node:path')
const { origin } = require('./session.cjs')
function features(window, emit, focus) {
  const icon = path.join(__dirname, '../icons/128x128.png')
  const tray = new Tray(nativeImage.createFromPath(icon).resize({ width: 20, height: 20 }))
  tray.setToolTip('Den'); tray.on('double-click', focus)
  let inCall = false, hook, held = false, paused = false, keycode = null
  const setHeld = value => { if (held !== value) { held = value; emit('ptt', value) } }
  function stopPTT() { if (hook) { hook.stop(); hook.removeAllListeners() }; hook = null; keycode = null; setHeld(false) }
  const pause = () => { paused = true; setHeld(false) }
  const resume = () => { paused = false }
  powerMonitor.on('suspend', pause)
  powerMonitor.on('lock-screen', pause)
  powerMonitor.on('resume', resume)
  powerMonitor.on('unlock-screen', resume)
  app.on('will-quit', stopPTT)
  window.webContents.on('render-process-gone', stopPTT)
  window.webContents.on('did-start-navigation', (_e, _url, inPlace, mainFrame) => { if (mainFrame && !inPlace) stopPTT() })
  function pttRegister(key) {
    stopPTT()
    if (!key) return
    if (!inCall) throw Error('Push-to-talk is available only during a call.')
    if (process.platform === 'darwin' && !systemPreferences.isTrustedAccessibilityClient(false)) throw Error('Allow Den in System Settings, Privacy & Security, Accessibility, then rejoin the call.')
    if (process.platform === 'linux' && process.env.XDG_SESSION_TYPE === 'wayland') throw Error('Global push-to-talk currently requires an X11 session. Use the focused-window shortcut on Wayland.')
    const { uIOhook, UiohookKey } = require('uiohook-napi')
    const aliases = { Backquote: 'Backquote', MetaLeft: 'Meta', MetaRight: 'MetaRight', ControlLeft: 'Ctrl', ControlRight: 'CtrlRight', ShiftLeft: 'Shift', ShiftRight: 'ShiftRight', AltLeft: 'Alt', AltRight: 'AltRight' }
    const mapped = aliases[key] || key.replace(/^Key|^Digit/, '')
    keycode = UiohookKey[mapped]
    if (!keycode) throw Error('This push-to-talk key is not supported.')
    hook = uIOhook
    hook.on('keydown', e => { if (!paused && e.keycode === keycode) setHeld(true) })
    hook.on('keyup', e => { if (e.keycode === keycode) setHeld(false) })
    try { hook.start() } catch { stopPTT(); throw Error('Cannot register global push-to-talk.') }
  }
  function trayState({ inCall: active, muted, deafened }) {
    inCall = !!active
    if (!inCall) stopPTT()
    tray.setContextMenu(Menu.buildFromTemplate([
      { label: 'Open Den', click: focus },
      { label: 'Mute microphone', type: 'checkbox', enabled: inCall, checked: !!muted, click: () => emit('tray-action', 'mute') },
      { label: 'Deafen', type: 'checkbox', enabled: inCall, checked: !!deafened, click: () => emit('tray-action', 'deafen') },
      { type: 'separator' }, { label: 'Quit', click: () => app.quit() },
    ]))
  }
  trayState({})
  const notifications = new Set()
  function notify({ title, body, origin: server, channel }) {
    server = origin(server)
    if (typeof title !== 'string' || title.length > 500 || typeof body !== 'string' || body.length > 1000 || typeof channel !== 'string' || channel.length > 100) throw Error('Invalid notification')
    const notification = new Notification({ title, body, icon, silent: true })
    notifications.add(notification)
    notification.once('click', () => { focus(); emit('notification-open', { origin: server, channel }) })
    notification.once('close', () => notifications.delete(notification))
    notification.show()
  }
  function badge(count) {
    if (!Number.isSafeInteger(count) || count < 0) throw Error('Invalid badge count')
    if (process.platform === 'darwin') app.dock.setBadge(count ? String(count) : '')
    else if (process.platform === 'win32') window.setOverlayIcon(count ? badgeImage(count) : null, count ? `${count} unread messages` : '')
    else app.setBadgeCount(count)
  }
  return { pttRegister, trayState, notify, badge, tray, stopPTT }
}
function badgeImage(count) {
  const pixels = Buffer.alloc(32 * 32 * 4)
  const digits = [[7,5,5,5,7],[2,6,2,2,7],[7,1,7,4,7],[7,1,7,1,7],[5,5,7,1,1],[7,4,7,1,7],[7,4,7,5,7],[7,1,1,1,1],[7,5,7,5,7],[7,5,7,1,7]]
  for (let y=0;y<32;y++) for(let x=0;x<32;x++) if((x-16)**2+(y-16)**2<250) pixels.set([74,164,232,255],(y*32+x)*4)
  const label = String(Math.min(count,99))
  for (let n=0;n<label.length;n++) for(let y=0;y<5;y++) for(let x=0;x<3;x++) if(digits[Number(label[n])][y] & (1<<(2-x))) for(let dy=0;dy<3;dy++) for(let dx=0;dx<3;dx++) pixels.set([22,25,27,255],((8+y*3+dy)*32+(label.length===1?12:5)+n*12+x*3+dx)*4)
  return nativeImage.createFromBitmap(pixels, { width: 32, height: 32 })
}
module.exports = { features }
