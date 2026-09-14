// Two disposable den-server processes on 17900/17901 and Vite on 17902.
import { chromium } from 'playwright-core'
import { readFile, mkdir, writeFile } from 'node:fs/promises'
import { randomBytes } from 'node:crypto'
import assert from 'node:assert/strict'
const bases = ['http://127.0.0.1:17900', 'http://127.0.0.1:17901']
const password = randomBytes(24).toString('hex')
const admin = [], bob = [], channels = [], keychain = new Map(), commands = [], errors = []
let savedOrigins = []
async function api(i, method, path, body, token = admin[i]?.token) {
  const r = await fetch(bases[i] + path, { method, headers: { 'content-type': 'application/json', ...(token ? { authorization: `Bearer ${token}` } : {}) }, body: body === undefined ? undefined : JSON.stringify(body) })
  assert(r.ok, `${method} ${path}: ${r.status}`)
  return r.status === 204 ? null : r.json()
}
// Keep credentials outside git so a rerun can reuse its disposable databases.
const privateFile = '/mnt/storage/den-m4-smoke/credentials.json'
let credentials
try { credentials = JSON.parse(await readFile(privateFile, 'utf8')) } catch { credentials = { password } }
for (const [i, name] of ['one', 'two'].entries()) {
  try {
    const key = await readFile(`/mnt/storage/den-m4-smoke/${name}/bootstrap.key`, 'utf8')
    admin[i] = await api(i, 'POST', '/auth/init', { username: 'desktop_admin', password: credentials.password, bootstrap_token: key }, '')
  } catch { admin[i] = await api(i, 'POST', '/auth/login', { username: 'desktop_admin', password: credentials.password }, '') }
  await writeFile(privateFile, JSON.stringify(credentials), { mode: 0o600 })
  await api(i, 'PUT', '/settings', { instance_name: i ? 'Second Den' : 'First Den' })
  try { bob[i] = await api(i, 'POST', '/auth/login', { username: 'desktop_bob', password: credentials.password }, '') }
  catch {
    const inv = await api(i, 'POST', '/invites', { uses: 1, expires_in_hours: 1 })
    bob[i] = await api(i, 'POST', '/auth/register', { username: 'desktop_bob', password: credentials.password, invite: inv.code }, '')
  }
  channels[i] = (await api(i, 'GET', '/channels')).find(c => c.kind === 'text')
  await api(i, 'PUT', '/users/me/appearance', { mode: 'dark', theme: i ? 'tide' : 'den', custom_themes: [] })
}
const browser = await chromium.launch({ executablePath: '/usr/bin/chromium', args: ['--no-sandbox'] })
const context = await browser.newContext({ viewport: { width: 1300, height: 850 } })
await context.exposeFunction('denInvoke', async (command, args = {}) => {
  commands.push({ command, origin: args.origin, path: args.path })
  if (command === 'session_set') { keychain.set(args.origin, args.token); return }
  if (command === 'session_get') return keychain.get(args.origin) || null
  if (command === 'session_clear') { keychain.delete(args.origin); return }
  if (command === 'instances_get') return savedOrigins
  if (command === 'instances_set') { savedOrigins = args.origins; return }
  if (command === 'platform') return 'linux'
  if (command === 'deep_links') return []
  if (command === 'update_check') return false
  if (command === 'api_request') {
    assert(bases.includes(args.origin))
    const token = keychain.get(args.origin)
    const r = await fetch(args.origin + args.path, { method: args.method, headers: { ...args.headers, ...(token ? { authorization: `Bearer ${token}` } : {}) }, body: args.body ? new Uint8Array(args.body) : undefined })
    return { status: r.status, headers: Object.fromEntries(r.headers), body: Array.from(new Uint8Array(await r.arrayBuffer())) }
  }
})
await context.addInitScript(origin => {
  if (!localStorage.getItem('den.native.origin')) localStorage.setItem('den.native.origin', origin)
  window.__TAURI__ = { core: { invoke: (c, a) => window.denInvoke(c, a), convertFileSrc: () => 'den-media://localhost/' }, event: { listen: async () => () => {} } }
}, bases[0])
const page = await context.newPage()
page.on('pageerror', e => errors.push(e.message))
const sockets = new Set()
page.on('websocket', socket => { sockets.add(socket); socket.on('close', () => sockets.delete(socket)) })
const until = async fn => { for (let n = 0; n < 100; n++) { if (await fn()) return; await page.waitForTimeout(100) } throw Error('Timed out') }
try {
  await page.goto('http://localhost:17902')
  await page.getByLabel('Username', { exact: true }).fill('desktop_admin')
  await page.getByLabel('Password', { exact: true }).fill(credentials.password)
  await page.getByRole('button', { name: 'Come in', exact: true }).click()
  await page.getByRole('button', { name: 'Switch server' }).waitFor()
  await until(() => commands.some(c => c.path === '/auth/ws-ticket' && c.origin === bases[0]))
  await page.getByRole('button', { name: 'Switch server' }).click()
  await page.getByRole('button', { name: 'Add a server' }).click()
  await page.getByLabel('Server URL').fill(bases[1])
  await page.getByRole('button', { name: 'Continue', exact: true }).click()
  await page.getByRole('heading', { name: 'Second Den' }).waitFor()
  await page.getByLabel('Username', { exact: true }).fill('desktop_admin')
  await page.getByLabel('Password', { exact: true }).fill(credentials.password)
  await page.getByRole('button', { name: 'Log in', exact: true }).click()
  await until(async () => (await page.getByRole('button', { name: 'Switch server' }).textContent()).includes('Second Den'))
  await until(() => commands.some(c => c.path === '/auth/ws-ticket' && c.origin === bases[1]))
  await page.keyboard.press('Control+Shift+Digit1')
  await until(async () => (await page.getByRole('button', { name: 'Switch server' }).textContent()).includes('First Den'))
  assert.equal(await page.evaluate(() => document.documentElement.style.getPropertyValue('--accent')), '#e8a44a')
  await page.keyboard.press('Control+Shift+BracketRight')
  await until(async () => (await page.getByRole('button', { name: 'Switch server' }).textContent()).includes('Second Den'))
  assert.equal(await page.evaluate(() => document.documentElement.style.getPropertyValue('--accent')), '#5fd3c6')
  await page.getByRole('link', { name: /^Inbox/ }).click()
  for (let i = 0; i < 2; i++) await api(i, 'POST', `/channels/${channels[i].id}/messages`, { content: `@desktop_admin Desktop smoke ${i} ${Date.now()}` }, bob[i].token)
  await until(async () => /First Den/i.test(await page.locator('section.inbox').innerText()) && /Second Den/i.test(await page.locator('section.inbox').innerText()))
  assert.match(await page.locator('section.inbox').innerText(), /First Den/i)
  assert.match(await page.locator('section.inbox').innerText(), /Second Den/i)
  await mkdir('docs/shots', { recursive: true })
  await page.screenshot({ path: 'docs/shots/m4-native-stub-inbox.png' })
  await page.getByRole('button', { name: 'Switch server' }).click()
  await page.screenshot({ path: 'docs/shots/m4-native-stub-switcher.png' })
  await page.getByRole('button', { name: 'Close server switcher' }).click()
  await page.keyboard.press('Control+k')
  await page.getByPlaceholder('Jump to a room, a person, or an action').fill('general')
  assert.equal(await page.locator('.palette .hint').filter({ hasText: /First Den|Second Den/ }).count(), 2)
  await page.keyboard.press('Escape')
  await page.reload()
  await page.getByRole('button', { name: 'Switch server' }).waitFor()
  await until(() => bases.every(origin => [...sockets].some(s => s.url().startsWith(origin.replace('http', 'ws') + '/ws?ticket='))))
  const storage = await page.evaluate(() => JSON.stringify(localStorage))
  for (const token of keychain.values()) assert(!storage.includes(token), 'Bearer token leaked into localStorage')
  assert.equal(keychain.size, 2)
  await page.getByRole('button', { name: 'Hide sidebar', exact: true }).click()
  for (const path of ['/inbox', '/find?q=desktop', '/settings', `/c/${channels[1].id}`]) {
    await page.goto('http://localhost:17902' + path)
    await page.getByRole('button', { name: 'Show sidebar', exact: true }).waitFor()
  }
  await page.screenshot({ path: 'docs/shots/m4-sidebar-collapsed.png' })
  await page.getByRole('button', { name: 'Show sidebar', exact: true }).click()
  await page.getByRole('button', { name: 'Hide sidebar', exact: true }).waitFor()
  await page.getByRole('button', { name: 'Switch server' }).click()
  await page.locator('.server.active .remove').click()
  await until(() => keychain.size === 1)
  assert.equal(keychain.size, 1)
  assert.equal(savedOrigins.length, 1)
  assert.deepEqual(errors, [])
  console.log('PASS native login, bearer-only HTTP, two ticket WebSockets, switcher, shortcuts, per-origin appearance, merged inbox, cross-server palette, resume and keychain logout')
} finally { await browser.close() }
