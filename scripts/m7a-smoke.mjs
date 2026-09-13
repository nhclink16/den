import { chromium } from 'playwright-core'
import { readFile, mkdir, mkdtemp, writeFile, rm } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { execFileSync } from 'node:child_process'
import assert from 'node:assert/strict'

const base = process.env.DEN_SMOKE_URL || 'http://localhost:5173'
const credentials = JSON.parse(await readFile(process.env.DEN_SMOKE_CREDENTIALS || `${process.env.HOME}/.local/share/den-dev/credentials.json`, 'utf8'))
const password = (user) => credentials.users?.[user] || credentials.password
const shots = new URL(process.env.DEN_SMOKE_SHOTS || '../docs/shots/', import.meta.url)
await mkdir(shots, { recursive: true })
const scratch = await mkdtemp(`${tmpdir()}/den-m7a-`)
async function api(method, path, body, token) {
  const r = await fetch(base + path, { method, headers: { 'Content-Type': 'application/json', ...(token ? { Authorization: `Bearer ${token}` } : {}) }, body: body === undefined ? undefined : JSON.stringify(body) })
  assert(r.ok, `${method} ${path}: ${r.status} ${r.ok ? '' : await r.text()}`)
  return r.status === 204 ? null : r.json()
}
const admin = await api('POST', '/auth/login', { username: 'nicholas', password: password('nicholas') })
let users = await api('GET', '/users', undefined, admin.token)
if (!users.some((u) => u.username === 'm6_bob')) {
  const invite = await api('POST', '/invites', { uses: 1, expires_in_hours: 1 }, admin.token)
  await api('POST', '/auth/register', { username: 'm6_bob', password: password('m6_bob'), invite: invite.code })
}
let bot = users.find((u) => u.username === 'clanker')
let credential
if (bot) credential = await api('POST', '/tokens', { name: 'm7a-smoke', user_id: bot.id }, admin.token)
else { const b = await api('POST', '/bots', { username: 'clanker', display_name: 'Clanker' }, admin.token); bot = b.user; credential = b.credential }
const browser = await chromium.launch({ executablePath: '/usr/bin/chromium', headless: true, args: [
  '--no-sandbox', '--use-fake-ui-for-media-stream', '--use-fake-device-for-media-stream',
  '--autoplay-policy=no-user-gesture-required', '--enable-usermedia-screen-capturing', '--auto-select-desktop-capture-source=Entire screen',
] })
const errors = []
async function until(check, label, timeout = 15_000) {
  const end = Date.now() + timeout
  while (Date.now() < end) { if (await check()) return; await new Promise((r) => setTimeout(r, 80)) }
  throw new Error(`Timed out: ${label}`)
}
async function login(username) {
  const context = await browser.newContext({ viewport: { width: 1440, height: 900 }, permissions: ['microphone', 'camera'] })
  await context.addInitScript(() => {
    window.__m7sockets = []
    const Original = WebSocket
    window.WebSocket = class extends Original { constructor(...args) { super(...args); window.__m7sockets.push(this) } }
  })
  const page = await context.newPage()
  page.on('pageerror', (err) => errors.push(err.message))
  await page.goto(base)
  await page.getByLabel('Username', { exact: true }).fill(username)
  await page.getByLabel('Password', { exact: true }).fill(password(username))
  await page.getByRole('button', { name: 'Come in', exact: true }).click()
  await page.locator('nav.side a.row').filter({ hasText: 'general' }).click()
  return page
}
const host = (p) => p.getByTestId('canvas-editor')
const ready = (p) => until(() => host(p).evaluate((el) => !!el.denEditor), 'mounted tldraw editor')
const shapes = (p) => host(p).evaluate((el) => el.denEditor.store.allRecords().filter((r) => r.typeName === 'shape'))
const shot = async (p, name) => { await p.evaluate(() => new Promise((r) => requestAnimationFrame(() => requestAnimationFrame(r)))); await p.screenshot({ path: new URL(`m7a-${name}.png`, shots).pathname }) }
let a, b
try {
  a = await login('nicholas'); b = await login('m6_bob'); console.log('Signed in both browsers')
  const room = (await api('GET', '/channels', undefined, admin.token)).find((c) => c.name === 'general')
  const prior = new Set((await api('GET', `/channels/${room.id}/messages`, undefined, admin.token)).flatMap((m) => m.objects || []).map((o) => o.id))
  const composer = a.getByRole('textbox', { name: 'Say something in #general' })
  await composer.fill('/')
  await a.getByRole('listbox', { name: 'Commands' }).waitFor()
  await composer.press('ArrowDown'); await composer.press('Enter')
  assert.equal(await composer.inputValue(), '/canvas ')
  await composer.fill('/canvas smoke'); await composer.press('Enter')
  await until(async () => (await api('GET', `/channels/${room.id}/messages`, undefined, admin.token)).some((m) => m.objects?.some((o) => o.name === 'smoke' && !prior.has(o.id))), 'canvas card posted')
  const messages = await api('GET', `/channels/${room.id}/messages`, undefined, admin.token)
  const object = messages.flatMap((m) => m.objects || []).filter((o) => o.name === 'smoke' && !prior.has(o.id)).at(-1)
  const card = (p) => p.locator(`[data-object-id="${object.id}"]`)
  await card(a).click(); await card(b).click(); await ready(a); await ready(b)
  await until(async () => (await api('GET', '/presence', undefined, admin.token)).objects.find((o) => o.id === object.id)?.user_ids.length === 2, 'two people present')
  console.log('Both canvases open, presence confirmed')
  const before = (await shapes(b)).length
  await host(a).evaluate((el) => el.denEditor.setCurrentTool('geo'))
  const canvas = host(a).locator('.tl-canvas')
  const box = await canvas.boundingBox()
  assert(box && box.width > 200 && box.height > 200)
  // Real pointer path through tldraw's event handlers, not createShapes().
  await a.mouse.move(box.x + 120, box.y + 120)
  await a.mouse.down(); await a.mouse.move(box.x + 320, box.y + 240, { steps: 8 }); await a.mouse.up()
  await until(async () => (await shapes(b)).length > before, 'rectangle reaches Bob', 2000)
  await host(b).evaluate((el) => el.denEditor.setCurrentTool('select'))
  const bb = await host(b).boundingBox()
  await b.mouse.move(bb.x + 200, bb.y + 140, { steps: 4 })
  await until(() => host(a).evaluate((el) => el.denEditor.getCollaborators().some((p) => p.userName.toLowerCase().includes('bob') && p.cursor)), 'Bob cursor in tldraw', 2000)
  await until(async () => await host(a).locator('.tl-cursor').count() > 0, 'visible collaborator cursor', 2000)
  console.log('Pointer rectangle and remote cursor passed')
  const pageId = await host(a).evaluate((el) => el.denEditor.getCurrentPageId())
  const shapeId = `shape:clanker-${Date.now()}`
  const patch = { base_version: 0, put: [{ id: shapeId, typeName: 'shape', type: 'text', x: 140, y: 265, rotation: 0, index: 'a2', parentId: pageId, isLocked: false, opacity: 1, meta: {}, props: { color: 'black', size: 'm', font: 'draw', textAlign: 'start', w: 280, richText: { type: 'doc', content: [{ type: 'paragraph', content: [{ type: 'text', text: 'Clanker: hold this corner' }] }] }, scale: 1, autoSize: true } }], remove: [] }
  await writeFile(`${scratch}/patch.json`, JSON.stringify(patch))
  const cli = process.env.DEN_SMOKE_CLI || './target/debug/den'
  const env = { ...process.env, DEN_URL: base, DEN_TOKEN: credential.token, DEN_CONFIG: `${scratch}/cli.json` }
  const result = JSON.parse(execFileSync(cli, ['canvas', 'patch', object.id, '--file', `${scratch}/patch.json`], { env, encoding: 'utf8' }))
  assert(result.version > 0)
  for (const p of [a, b]) {
    await until(async () => (await shapes(p)).some((s) => s.id === shapeId), 'CLI text reaches both editors', 2000)
    await host(p).locator('.tl-shape').filter({ hasText: 'Clanker: hold this corner' }).first().waitFor()
  }
  await until(async () => { const o = await api('GET', `/objects/${object.id}/summary`, undefined, admin.token); return !!o.thumbnail_url }, 'thumbnail within 15 seconds', 15_000)
  await until(() => card(a).locator('img').evaluate((el) => el.complete && el.naturalWidth === 640).catch(() => false), '640px thumbnail rendered')
  for (const p of [a, b]) await host(p).evaluate((el) => { el.denEditor.selectNone(); el.denEditor.zoomToFit({ animation: { duration: 0 } }) })
  console.log('CLI shape and 640px thumbnail passed')
  await shot(a, 'docked-desktop')
  await a.getByRole('button', { name: 'Expand canvas', exact: true }).click(); await shot(a, 'expanded-desktop')
  await a.getByRole('button', { name: 'Collapse canvas', exact: true }).click()
  // A socket gap must reload the full document and rejoin object presence.
  await a.context().setOffline(true)
  await a.evaluate(() => window.__m7sockets.at(-1).close())
  const moved = { ...patch, put: [{ ...patch.put[0], x: 170 }] }
  execFileSync(cli, ['canvas', 'patch', object.id, '--file', '-'], { env, input: JSON.stringify(moved), encoding: 'utf8' })
  await until(async () => (await shapes(b)).find((s) => s.id === shapeId)?.x === 170, 'Bob receives edit during Nicholas gap')
  await a.context().setOffline(false)
  await until(() => a.evaluate(() => window.__m7sockets.at(-1).readyState === WebSocket.OPEN && window.__m7sockets.length > 1), 'websocket reconnect')
  await until(async () => (await shapes(a)).find((s) => s.id === shapeId)?.x === 170, 'missed edit restored by resync')
  await a.getByRole('button', { name: 'Close canvas', exact: true }).click()
  await card(a).scrollIntoViewIfNeeded(); await shot(a, 'card-desktop')
  await card(a).click(); await ready(a)
  await a.locator('nav.side').getByRole('button', { name: 'Join hangout', exact: true }).click()
  await a.getByTestId('call-dock').waitFor()
  await until(async () => await a.getByRole('button', { name: 'Turn camera on', exact: true }).isEnabled(), 'call ready')
  await a.getByRole('button', { name: 'Turn camera on', exact: true }).click()
  await until(() => a.getByTestId('call-tile').locator('video').evaluateAll((vs) => vs.some((v) => v.videoWidth > 0)), 'camera decoded')
  await a.getByRole('button', { name: 'Expand call', exact: true }).click()
  await a.getByTestId('canvas-call-tile').waitFor()
  await until(() => a.getByTestId('canvas-tile-editor').evaluate((el) => !!el.denEditor && el.denEditor.getInstanceState().isReadonly), 'read-only canvas in call grid')
  await shot(a, 'call-grid-desktop')
  await a.setViewportSize({ width: 390, height: 844 }); await shot(a, 'call-grid-mobile')
  await a.getByRole('button', { name: 'Collapse call', exact: true }).click()
  await a.getByRole('button', { name: 'Leave call', exact: true }).click()
  await shot(a, 'docked-mobile')
  await a.getByRole('button', { name: 'Expand canvas', exact: true }).click(); await shot(a, 'expanded-mobile')
  await a.getByRole('button', { name: 'Close canvas', exact: true }).click()
  await card(a).scrollIntoViewIfNeeded(); await shot(a, 'card-mobile')
  await until(async () => (await api('GET', '/presence', undefined, admin.token)).objects.find((o) => o.id === object.id)?.user_ids.length === 1, 'closing canvas removes only Nicholas')
  await b.getByRole('button', { name: 'Close canvas', exact: true }).click()
  const saved = JSON.parse(execFileSync(cli, ['canvas', 'get', object.id], { env, encoding: 'utf8' }))
  assert(saved[shapeId])
  assert(Object.values(saved).every((r) => ['document','page','shape','binding','asset'].includes(r.typeName)))
  // Registry palette action and admin-only settings UI.
  await a.setViewportSize({ width: 1440, height: 900 })
  await a.keyboard.press('Control+k')
  await a.getByRole('dialog', { name: 'Go to' }).getByRole('textbox').fill('New canvas')
  await a.getByRole('button', { name: 'New canvas in #general canvas' }).waitFor()
  await a.keyboard.press('Escape')
  await a.goto(base + '/settings/plugins')
  await a.getByRole('checkbox', { name: 'Canvas', exact: true }).waitFor()
  await b.goto(base + '/settings')
  await until(async () => await b.getByRole('heading', { name: 'Settings', exact: true }).count() === 1, 'settings page navigation')
  assert.equal(await b.getByRole('link', { name: 'Plugins', exact: true }).count(), 0)
  assert.deepEqual(errors, [])
  console.log(`M7a smoke passed: ${base}; object ${object.id}; pointer drawing, cursors, CLI bot, thumbnail, resync, presence, call tile, eight screenshots`)
} catch (err) {
  if (a) await shot(a, 'failure')
  console.error('Browser errors:', errors)
  throw err
} finally {
  await browser.close()
  await api('DELETE', `/tokens/${credential.credential.id}`, undefined, admin.token)
  await rm(scratch, { recursive: true, force: true })
}
