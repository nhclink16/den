const fs = require('node:fs')
const path = require('node:path')

function origin(value) {
  const u = new URL(value)
  if (u.username || u.password || u.search || u.hash || u.pathname !== '/' || !(u.protocol === 'https:' || (u.protocol === 'http:' && ['localhost', '127.0.0.1', '[::1]'].includes(u.hostname)))) throw Error('Use an HTTPS origin, or localhost for development.')
  return u.origin
}
function requestUrl(server, route) {
  const base = origin(server)
  if (typeof route !== 'string' || !route.startsWith('/') || route.startsWith('//')) throw Error('Invalid API path')
  const url = new URL(route, base)
  if (url.origin !== base) throw Error('API request changed origin')
  return url
}
function storage(directory, safeStorage) {
  fs.mkdirSync(directory, { recursive: true, mode: 0o700 })
  const file = path.join(directory, 'native.json')
  let state = { origins: [], sessions: {} }
  try { state = JSON.parse(fs.readFileSync(file, 'utf8')) } catch (e) { if (e.code !== 'ENOENT') throw Error('Cannot read native session storage') }
  function save() { fs.writeFileSync(file + '.tmp', JSON.stringify(state), { mode: 0o600 }); fs.renameSync(file + '.tmp', file) }
  function unlocked() {
    if (!safeStorage.isEncryptionAvailable() || (process.platform === 'linux' && safeStorage.getSelectedStorageBackend() === 'basic_text')) throw Error('Unlock your OS keychain to save or restore your Den session.')
  }
  return {
    get(server) { const encrypted = state.sessions[origin(server)]; if (!encrypted) return null; unlocked(); return safeStorage.decryptString(Buffer.from(encrypted, 'base64')) },
    set(server, token) { server = origin(server); if (typeof token !== 'string' || !token || token.length > 4096) throw Error('Invalid session'); unlocked(); state.sessions[server] = safeStorage.encryptString(token).toString('base64'); save() },
    clear(server) { delete state.sessions[origin(server)]; save() },
    origins() { return state.origins },
    remember(origins) { if (!Array.isArray(origins) || origins.length > 100) throw Error('Invalid server list'); state.origins = [...new Set(origins.map(origin))]; save() },
  }
}
async function request(store, server, method, route, body, headers = {}) {
  const url = requestUrl(server, route), outgoing = {}
  if (!['GET', 'HEAD', 'POST', 'PUT', 'PATCH', 'DELETE'].includes(method)) throw Error('Invalid method')
  for (const [key, value] of Object.entries(headers)) if (['content-type', 'range', 'upload-offset', 'if-range'].includes(key.toLowerCase())) outgoing[key.toLowerCase()] = value
  if (!['/auth/login', '/auth/register', '/instance'].includes(url.pathname)) {
    const token = store.get(server)
    if (token) outgoing.authorization = `Bearer ${token}`
  }
  try { return await fetch(url, { method, headers: outgoing, body: body == null ? undefined : Buffer.from(body), redirect: 'error', signal: AbortSignal.timeout(60000) }) }
  catch { throw Error('Cannot reach this server') }
}
async function apiRequest(store, args) {
  const r = await request(store, args.origin, args.method, args.path, args.body, args.headers)
  return { status: r.status, headers: Object.fromEntries([...r.headers].filter(([k]) => !['set-cookie', 'authorization'].includes(k))), body: Array.from(new Uint8Array(await r.arrayBuffer())) }
}
// Images the renderer may load with this device's stored session: attachments,
// profile pictures and banners, and wallpapers. Nothing that changes state.
const mediaPath = path => /^\/(uploads\/[^/]+\/(file|thumbnail)|users\/[^/]+\/(avatar|banner)|users\/me\/background\/image|users\/me\/backgrounds\/[0-9a-f]{64}(\/preview)?)$/.test(path)
async function media(store, req) {
  try {
    const u = new URL(req.url)
    if (!mediaPath(u.pathname) || !['GET', 'HEAD'].includes(req.method)) return new Response(null, { status: 400 })
    const r = await request(store, u.searchParams.get('origin'), req.method, u.pathname, null, Object.fromEntries(req.headers))
    const headers = { 'access-control-allow-origin': 'den://app', 'cache-control': 'no-store' }
    for (const key of ['content-type', 'content-length', 'content-range', 'accept-ranges', 'content-disposition']) if (r.headers.has(key)) headers[key] = r.headers.get(key)
    return new Response(r.body, { status: r.status, headers })
  } catch { return new Response(null, { status: 502 }) }
}
module.exports = { origin, requestUrl, storage, request, apiRequest, media, mediaPath }
