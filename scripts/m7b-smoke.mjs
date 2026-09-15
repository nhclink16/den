import { SmokeCleanup, keepSmoke } from './smoke-cleanup.mjs'
import { chromium } from 'playwright-core'
import { readFile, mkdir, mkdtemp, rm } from 'node:fs/promises'
import { execFileSync, spawn } from 'node:child_process'
import { once } from 'node:events'
import assert from 'node:assert/strict'
const base = process.env.DEN_SMOKE_URL || 'http://localhost:5173'
const credentials = JSON.parse(await readFile(process.env.DEN_SMOKE_CREDENTIALS || `${process.env.HOME}/.local/share/den-dev/credentials.json`, 'utf8'))
const password = user => credentials.users?.[user] || credentials.password
const hostBin = process.env.DEN_SMOKE_HOST_BIN || new URL('../target/debug/den-host', import.meta.url).pathname
const shots = new URL(process.env.DEN_SMOKE_SHOTS || '../docs/shots/', import.meta.url)
await mkdir(shots, { recursive: true })
async function api(method, path, body, token) {
  const r = await fetch(base + path, { method, headers: { 'Content-Type': 'application/json', ...(token ? { Authorization: `Bearer ${token}` } : {}) }, body: body === undefined ? undefined : JSON.stringify(body) })
  assert(r.ok, `${method} ${path}: ${r.status}`); return r.status === 204 ? null : r.json()
}
async function until(check, label, timeout = 15000) {
  const end = Date.now() + timeout
  while (Date.now() < end) { if (await check()) return; await new Promise(r => setTimeout(r, 80)) }
  throw Error(`Timed out: ${label}`)
}
const admin = await api('POST', '/auth/login', { username: 'nicholas', password: password('nicholas') })
const cleanup = await SmokeCleanup.start(base, admin.token)
const hostName = `m7b-smoke-${Date.now()}`
const scratch = await mkdtemp('/mnt/storage/den-m7b-smoke-')
let hostProcess, host, browser
try {
  const enrollment = await api('POST', '/hosts/enroll', {}, admin.token)
  const env = { ...process.env, DEN_HOST_CONFIG_DIR: scratch, DEN_HOST_NAME: hostName }
  execFileSync(hostBin, ['login', enrollment.code], { env, stdio: ['ignore','ignore','pipe'] })
  host = (await api('GET', '/hosts', undefined, admin.token)).find(h => h.name === hostName)
  assert(host, 'temporary host enrolled')
  hostProcess = spawn(hostBin, ['run'], { env, stdio: 'ignore' })
  await until(async () => (await api('GET', '/hosts', undefined, admin.token)).some(h => h.id === host.id && h.online), 'temporary host connected')
let users = await api('GET', '/users', undefined, admin.token)
if (!users.some(u => u.username === 'm6_bob')) {
  const invite = await api('POST', '/invites', { uses: 1, expires_in_hours: 1 }, admin.token)
  await api('POST', '/auth/register', { username: 'm6_bob', password: password('m6_bob'), invite: invite.code })
}
const bob = await api('POST', '/auth/login', { username: 'm6_bob', password: password('m6_bob') })
const self = await api('POST', '/dms', { member_ids: [admin.user.id] }, admin.token)
const dm = await api('POST', '/dms', { member_ids: [bob.user.id] }, admin.token)
const general = (await api('GET', '/channels', undefined, admin.token)).find(c => c.name === 'general')
browser = await chromium.launch({ executablePath: '/usr/bin/chromium', headless: true, args: ['--no-sandbox', '--use-fake-ui-for-media-stream', '--use-fake-device-for-media-stream', '--autoplay-policy=no-user-gesture-required'] })
const errors = []
async function login(session) {
  const context = await browser.newContext({ viewport: { width: 1440, height: 900 }, permissions: ['microphone', 'camera'] })
  cleanup.watch(context)
  await context.addCookies([{ name: 'den_session', value: session.token, url: base, httpOnly: true, secure: base.startsWith('https:'), sameSite: 'Lax' }])
  await context.addInitScript(csrf => localStorage.setItem('den.csrf', csrf), session.csrf_token)
  if (process.env.DEN_SMOKE_RELAY === '1') await context.routeWebSocket(/\.ts\.net:/, ws => ws.close());
  const page = await context.newPage(); page.on('pageerror', e => errors.push(e.message)); return page
}
const editor = p => p.getByTestId('terminal-editor')
const ready = p => until(() => editor(p).evaluate(el => !!el.denTerminal), 'Ghostty mounted')
const screen = async p => (await editor(p).count()) ? editor(p).evaluate(el => { const t = el.denTerminal; const b = t.buffer.active; return Array.from({ length: b.length }, (_,i) => b.getLine(i)?.translateToString(true) || '').join('\n') }) : ''
async function type(p, text) {await editor(p).evaluate(el => el.denTerminal.focus()); await p.keyboard.type(text); await p.keyboard.press('Enter')}
const shot = async (p, name) => { await p.evaluate(() => new Promise(r => requestAnimationFrame(() => requestAnimationFrame(r)))); await p.screenshot({ path: new URL(`m7b-${name}.png`, shots).pathname }) }
async function pairShots(p, name) { await shot(p, `${name}-desktop`); await p.setViewportSize({ width: 390, height: 844 }); await p.waitForTimeout(350); await shot(p, `${name}-mobile`); await p.setViewportSize({ width: 1440, height: 900 }); await p.waitForTimeout(300) }
let a, b, id, sharedId
try {
  a = await login(admin); b = await login(bob)
  await a.goto(`${base}/c/${self.id}`)
  await a.getByRole('textbox', { name: 'Message Just you', exact: true }).fill(`/terminal ${hostName}`)
  await a.getByRole('textbox', { name: 'Message Just you', exact: true }).press('Enter')
  await editor(a).waitFor(); await ready(a)
  await a.getByRole('checkbox', { name: 'Record session', exact: true }).click()
  await until(async () => { const messages = await api('GET', `/channels/${self.id}/messages`, undefined, admin.token); const id = messages.flatMap(m => m.objects || []).filter(o => o.kind === 'terminal').at(-1)?.id; return id && (await api('GET', `/objects/${id}`, undefined, admin.token)).state.terminal.recording_enabled }, 'recording enabled')
  const messages = await api('GET', `/channels/${self.id}/messages`, undefined, admin.token)
  id = messages.flatMap(m => m.objects || []).filter(o => o.kind === 'terminal').at(-1).id
  await until(async () => (await screen(a)).includes('codexbox'), 'shell prompt')
  await type(a, "printf 'DEN_%s\\n' OK")
  await until(async () => (await screen(a)).includes('DEN_OK'), 'DEN_OK in Ghostty')
  const direct = await a.locator('.view-status .direct').count() > 0
  if (process.env.DEN_SMOKE_RELAY === '1') assert(!direct, 'forced relay must not use direct transport')
  console.log(`Owner keyboard -> PTY -> Ghostty passed (${direct ? 'direct' : 'relay'})`)
  await a.getByLabel('Post terminal card to').selectOption(general.id); await a.getByRole('button', { name: 'Post', exact: true }).click(); await until(async () => (await a.getByLabel('Post terminal card to').inputValue()) === '', 'shared card posted')
  const generalMessages = await api('GET', `/channels/${general.id}/messages`, undefined, admin.token)
  sharedId = generalMessages.flatMap(m => m.objects || []).filter(o => o.kind === 'terminal').at(-1).id
  await b.goto(`${base}/c/${general.id}`); await b.locator(`[data-object-id="${sharedId}"]`).click(); await ready(b)
  await b.getByRole('button', { name: 'Request access', exact: true }).click()
  await a.getByRole('button', { name: 'Close terminal', exact: true }).click(); await a.goto(`${base}/c/${dm.id}`)
  const request = a.getByTestId('access-request-card').last(); await request.getByRole('button', { name: 'Allow', exact: true }).waitFor()
  await pairShots(a, 'access-request'); await request.getByRole('button', { name: 'Allow', exact: true }).click(); await request.getByText('Allowed for 1 hour', { exact: true }).waitFor()
  // The existing viewer attaches as soon as the grant arrives.
  await until(async () => (await screen(b)).includes('DEN_OK'), 'Bob scrollback sees DEN_OK')
  await a.goto(`${base}/c/${general.id}`); await a.locator(`[data-object-id="${sharedId}"]`).click(); await ready(a)
  await until(async () => (await screen(a)).includes('DEN_OK'), 'owner reattaches before card preview'); await a.waitForTimeout(150)
  await a.getByRole('button', {name:'Close terminal',exact:true}).click()
  await a.locator(`[data-object-id="${sharedId}"]`).scrollIntoViewIfNeeded(); await pairShots(a, 'card')
  await a.locator(`[data-object-id="${sharedId}"]`).click(); await ready(a)
  await b.getByRole('button', { name: 'Request control', exact: true }).click()
  await a.getByRole('button', { name: 'Close terminal', exact: true }).click(); await a.goto(`${base}/c/${dm.id}`)
  await a.getByTestId('access-request-card').last().getByRole('button', { name: 'Allow', exact: true }).click(); await a.getByTestId('access-request-card').last().getByText('Allowed for 1 hour', { exact: true }).waitFor(); await b.waitForTimeout(300)
  await a.goto(`${base}/c/${general.id}`); await a.locator(`[data-object-id="${sharedId}"]`).click(); await ready(a)
  await b.getByRole('button', { name: 'Request control', exact: true }).click()
  await a.getByRole('button', {name:'Join hangout',exact:true}).click()
  await a.getByTestId('terminal-call-tile').getByRole('button', { name: 'Give', exact: true }).click()
  await until(async () => (await api('GET', `/objects/${id}`, undefined, admin.token)).state.terminal.active_controller_id === bob.user.id, 'owner promotes Bob')
  await b.getByTestId('terminal-control-banner').getByText('m6_bob has control', { exact: true }).waitFor()
  await b.getByText('View only', { exact: true }).waitFor({ state: 'hidden' })
  await type(b, 'echo BOB_OK')
  await until(async () => (await screen(a)).split('\n').some(l => l.trim() === 'BOB_OK') && (await screen(b)).split('\n').some(l => l.trim() === 'BOB_OK'), 'Bob input reaches both screens', 2000)
  await a.getByRole('button',{name:/Leave call/}).click()
  await pairShots(a, 'control-banner')
  console.log('View request, control request, owner promotion and Bob input passed')
  await a.getByRole('button', { name: 'Revoke control', exact: true }).click()
  await until(async () => (await api('GET', `/objects/${id}`, undefined, admin.token)).state.terminal.active_controller_id === admin.user.id, 'revoke takes effect', 1000)
  await type(b, 'echo REVOKED_SHOULD_NOT_APPEAR'); await b.waitForTimeout(2000)
  assert(!(await screen(a)).includes('REVOKED_SHOULD_NOT_APPEAR')); assert(!(await screen(b)).includes('REVOKED_SHOULD_NOT_APPEAR'))
  const denied = await fetch(`${base}/sessions/${id}/write`, {method:'POST',headers:{authorization:`Bearer ${bob.token}`,'content-type':'application/json'},body:JSON.stringify({text:'echo FORGED_INPUT\n'})}); assert.equal(denied.status,403)
  console.log('Revoked browser and forged API input both denied')
  await type(a, 'herdr --session den-m7b-smoke')
  await until(async () => (await screen(a)).includes('machines') && (await screen(a)).includes('Local'), 'Herdr TUI rendered')
  await type(a, "printf 'HERDR_%s\\n' OK")
  await until(async () => (await screen(a)).includes('HERDR_OK'), 'Herdr accepts keyboard input')
  await type(a, 'ssh -o BatchMode=yes -o ConnectTimeout=8 vps hostname');
  await until(async () => (await screen(a)).includes('vps-2fd9743a'), 'Herdr drives the VPS over SSH');
  await pairShots(a, 'docked-herdr'); await until(async () => (await screen(b)).includes('HERDR_OK'), 'Bob keeps Herdr screen across resize')
  await a.getByRole('button',{name:'Expand terminal',exact:true}).click(); await pairShots(a,'expanded-herdr'); await a.getByRole('button',{name:'Collapse terminal',exact:true}).click()
  // Focused terminal receives Ctrl+K. Esc Esc releases it, so Den gets the next Ctrl+K.
  await editor(a).evaluate(el=>el.denTerminal.focus()); await a.keyboard.press('Control+k'); assert.equal(await a.getByRole('dialog',{name:'Go to'}).count(),0)
  await a.keyboard.press('Escape'); await a.keyboard.press('Escape'); await a.keyboard.press('Control+k'); await a.getByRole('dialog',{name:'Go to'}).waitFor(); await a.keyboard.press('Escape')
  await a.getByRole('button', {name:'Join hangout',exact:true}).click()
  await a.getByTestId('terminal-call-tile').waitFor(); await pairShots(a,'call-tile')
  const leave=a.getByRole('button',{name:/Leave call/}); if(await leave.count()) await leave.click()
  await a.getByRole('button',{name:'End session',exact:true}).click()
  await a.getByTestId('terminal-replay').waitFor()
  const state=(await api('GET',`/objects/${id}`,undefined,admin.token)).state.terminal; assert(state.recording_upload_id)
  await until(async () => +(await a.getByLabel('Replay position').getAttribute('max')) > 1, 'recording downloaded')
  await a.getByLabel('Replay position').fill(String(await a.getByLabel('Replay position').getAttribute('max')))
  await until(async () => (await screen(a)).includes('vps-2fd9743a'), 'full replay renders Herdr and VPS output')
  // The complete recording includes BOB_OK even if Herdr later entered the alternate screen.
  const file=await fetch(`${base}/uploads/${state.recording_upload_id}/file`,{headers:{authorization:`Bearer ${admin.token}`}});assert(file.ok)
  const records=(await file.text()).trim().split('\n').map(JSON.parse)
  const bobTime=records.find(([ms,b64])=>Buffer.from(b64,'base64').toString().includes('BOB_OK'))?.[0];assert(bobTime!==undefined)
  await a.getByLabel('Replay position').fill(String(bobTime - records[0][0]));await until(async()=> (await screen(a)).includes('BOB_OK'),'recording replays BOB_OK')
  await pairShots(a,'replay')
  assert.equal(errors.length,0,errors.join('\n'))
  console.log(`PASS ${base}: session ${id}, shared card ${sharedId}; screenshots saved`)
} catch (e) {
  if (a) {await shot(a,'failure-owner'); console.error('Owner screen:',await screen(a).catch(()=>''))}
  if (b) {await shot(b,'failure-viewer'); console.error('Viewer screen:',await screen(b).catch(()=>''))}
  throw e
 }
} finally {
  try { await cleanup.finish(browser) }
  finally {
    if (hostProcess && hostProcess.exitCode === null && hostProcess.signalCode === null) {
      const stopped = once(hostProcess, 'exit')
      hostProcess.kill('SIGTERM')
      await stopped
    }
    try {
      if (!keepSmoke) {
        host ||= (await api('GET', '/hosts', undefined, admin.token)).find(h => h.name === hostName)
        if (host) await api('DELETE', `/hosts/${host.id}`, undefined, admin.token)
        await rm(scratch, { recursive: true, force: true })
      }
    } finally { await api('POST', '/auth/logout', {}, admin.token) }
  }
}
