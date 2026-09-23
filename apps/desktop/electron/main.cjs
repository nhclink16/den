const { app, BrowserWindow, ipcMain, protocol, net, session, desktopCapturer, dialog, Menu, shell, screen, safeStorage, powerMonitor } = require('electron')
const fs = require('node:fs')
const path = require('node:path')
const { pathToFileURL } = require('node:url')
const { storage, apiRequest, media } = require('./session.cjs')
const { features } = require('./features.cjs')
const { updater } = require('./updater.cjs')
const { activity } = require('./activity.cjs')
app.setName('Den')
app.setAppUserModelId('app.denchat.desktop')
if (process.env.DEN_DESKTOP_DATA) app.setPath('userData', path.resolve(process.env.DEN_DESKTOP_DATA))
protocol.registerSchemesAsPrivileged([
  { scheme: 'den', privileges: { standard: true, secure: true, supportFetchAPI: true, corsEnabled: true, codeCache: true } },
  { scheme: 'den-media', privileges: { standard: true, secure: true, supportFetchAPI: true, corsEnabled: true, stream: true } },
])
let window, quitting = false
const pendingLinks = process.argv.filter(link => link.startsWith('den://join?'))
function focus() { if (window) { window.show(); if (window.isMinimized()) window.restore(); window.focus() } }
function emit(event, payload) { if (window && !window.isDestroyed()) window.webContents.send(`den:${event}`, payload) }
function links(values) { const valid = values.filter(link => { try { const u = new URL(link); return u.protocol === 'den:' && u.hostname === 'join' } catch { return false } }); pendingLinks.push(...valid); emit('deep-link://new-url', valid); focus() }
if (!app.requestSingleInstanceLock()) app.quit()
else {
  app.on('second-instance', (_event, argv) => links(argv))
  app.on('open-url', (event, url) => { event.preventDefault(); links([url]) })
  app.on('activate', focus)
  app.on('before-quit', () => { quitting = true })
  app.whenReady().then(start).catch(() => { dialog.showErrorBox('Den could not start', 'Cannot initialize desktop storage or the app window.'); app.quit() })
}
async function start() {
  const store = storage(app.getPath('userData'), safeStorage)
  const dev = !app.isPackaged && process.env.DEN_DESKTOP_URL
  if (dev && !/^http:\/\/(localhost|127\.0\.0\.1):\d+$/.test(dev)) throw Error('Development URL must be localhost')
  const appUrl = dev || 'den://app'
  const trusted = url => { try { const u = new URL(url), base = new URL(appUrl); return u.protocol === base.protocol && u.host === base.host } catch { return false } }
  const webRoot = app.isPackaged ? path.join(process.resourcesPath, 'web') : path.resolve(__dirname, '../../web/dist')
  protocol.handle('den', async req => {
    const url = new URL(req.url)
    if (url.hostname !== 'app') return new Response(null, { status: 404 })
    const name = decodeURIComponent(url.pathname)
    const file = path.resolve(webRoot, '.' + name)
    if (file !== webRoot && !file.startsWith(webRoot + path.sep)) return new Response(null, { status: 403 })
    const target = fs.existsSync(file) && fs.statSync(file).isFile() ? file : path.join(webRoot, 'index.html')
    const r = await net.fetch(pathToFileURL(target).href)
    const headers = new Headers(r.headers)
    headers.set('Content-Security-Policy', "default-src 'self'; script-src 'self' blob: 'wasm-unsafe-eval'; style-src 'self' 'unsafe-inline' https://fonts.googleapis.com; font-src 'self' data: https://fonts.gstatic.com; connect-src 'self' den-media: https: wss: http://localhost:* http://127.0.0.1:* ws://localhost:* ws://127.0.0.1:*; img-src 'self' data: blob: den-media: https:; media-src 'self' blob: den-media:; worker-src 'self' blob:; frame-src 'none'; object-src 'none'; base-uri 'self'")
    return new Response(r.body, { status: r.status, headers })
  })
  protocol.handle('den-media', req => media(store, req))
  session.defaultSession.setPermissionCheckHandler((wc, permission, requestingOrigin) => wc === window?.webContents && trusted(requestingOrigin) && ['media', 'display-capture', 'fullscreen'].includes(permission))
  session.defaultSession.setPermissionRequestHandler((wc, permission, callback, details) => callback(wc === window?.webContents && trusted(details.requestingUrl || wc.getURL()) && ['media', 'display-capture', 'fullscreen'].includes(permission)))
  session.defaultSession.setDisplayMediaRequestHandler(async (request, callback) => {
    if (!request.frame || request.frame !== window?.webContents.mainFrame || !trusted(request.frame.url)) return callback({})
    try {
      const sources = await desktopCapturer.getSources({ types: ['screen', 'window'] })
      const answer = await dialog.showMessageBox(window, { title: 'Share your screen', message: 'Choose a screen or window to share', buttons: ['Cancel', ...sources.map(s => s.name)], cancelId: 0, defaultId: 0, noLink: true })
      if (!answer.response) return callback({})
      callback({ video: sources[answer.response - 1], ...(request.audioRequested && process.platform === 'win32' ? { audio: 'loopback' } : {}) })
    } catch { callback({}) }
  }, { useSystemPicker: true })
  const boundsFile = path.join(app.getPath('userData'), 'window.json')
  let bounds = { width: 1200, height: 800 }
  try {
    const saved = JSON.parse(fs.readFileSync(boundsFile, 'utf8'))
    if (Number.isInteger(saved.width) && Number.isInteger(saved.height) && saved.width >= 900 && saved.height >= 600 && screen.getAllDisplays().some(d => saved.x < d.workArea.x + d.workArea.width && saved.x + saved.width > d.workArea.x && saved.y < d.workArea.y + d.workArea.height && saved.y + saved.height > d.workArea.y)) bounds = saved
  } catch { /* first launch */ }
  window = new BrowserWindow({ ...bounds, minWidth: 900, minHeight: 600, title: 'Den', icon: path.join(__dirname, '../icons/icon.png'), ...(process.platform === 'darwin' ? { titleBarStyle: 'hiddenInset' } : {}), webPreferences: { preload: path.join(__dirname, 'preload.cjs'), contextIsolation: true, nodeIntegration: false, sandbox: true, webSecurity: true } })
  const native = features(window, emit, focus), updates = updater(), doing = activity(emit, powerMonitor)
  Menu.setApplicationMenu(process.platform === 'darwin' ? Menu.buildFromTemplate([{ role: 'appMenu' }, { role: 'editMenu' }, { role: 'windowMenu' }]) : null)
  window.on('close', event => { fs.writeFileSync(boundsFile, JSON.stringify(window.getNormalBounds()), { mode: 0o600 }); if (!quitting) { event.preventDefault(); window.hide() } })
  window.on('blur', () => emit('window-background', null))
  window.webContents.on('will-navigate', (event, url) => { if (!trusted(url)) event.preventDefault() })
  window.webContents.on('will-redirect', (event, url) => { if (!trusted(url)) event.preventDefault() })
  window.webContents.setWindowOpenHandler(({ url }) => {
    // Call pop-outs use a blank, same-origin child and keep their mounted media nodes.
    if (url === 'about:blank') return { action: 'allow', overrideBrowserWindowOptions: { webPreferences: { nodeIntegration: false, contextIsolation: true, sandbox: true, preload: undefined } } }
    if (/^https?:\/\//.test(url)) void shell.openExternal(url)
    return { action: 'deny' }
  })
  window.webContents.on('did-create-window', child => { child.webContents.on('will-navigate', event => event.preventDefault()); child.webContents.setWindowOpenHandler(() => ({ action: 'deny' })) })
  const commands = {
    platform: () => ({ darwin: 'macos', win32: 'windows', linux: 'linux' })[process.platform],
    session_get: a => store.get(a.origin), session_set: a => store.set(a.origin, a.token), session_clear: a => store.clear(a.origin),
    instances_get: () => store.origins(), instances_set: a => store.remember(a.origins), api_request: a => apiRequest(store, a),
    ptt_register: a => native.pttRegister(a.key), tray_state: a => native.trayState(a), notify: a => native.notify(a), badge: a => native.badge(a.count),
    deep_links: () => pendingLinks.splice(0), update_check: () => app.isPackaged ? updates.check() : false, update_restart: () => updates.restart(),
    activity_current: () => doing.current(),
  }
  ipcMain.handle('den:command', (event, command, args = {}) => {
    if (event.sender !== window.webContents || event.senderFrame !== window.webContents.mainFrame || !trusted(event.senderFrame.url) || !Object.hasOwn(commands, command)) throw Error('Desktop command denied')
    return commands[command](args)
  })
  if (app.isPackaged) app.setAsDefaultProtocolClient('den')
  await window.loadURL(appUrl + '/')
}
