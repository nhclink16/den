// Run after the release installers have enrolled the named host.
// DEN_SMOKE_HOST=<name> node scripts/m8-host-smoke.mjs
import { chromium } from 'playwright-core'
import { readFile, mkdir } from 'node:fs/promises'
import assert from 'node:assert/strict'

const base = process.env.DEN_SMOKE_URL || 'https://denchat.app'
const hostName = process.env.DEN_SMOKE_HOST
assert(hostName, 'Set DEN_SMOKE_HOST to the enrolled machine name')
const credentials = JSON.parse(await readFile(process.env.DEN_SMOKE_CREDENTIALS || `${process.env.HOME}/.local/share/den-m6/smoke-credentials.json`, 'utf8'))
async function api(method, path, body, token) {
  const r = await fetch(base + path, { method, headers: { 'content-type': 'application/json', ...(token ? { authorization: `Bearer ${token}` } : {}) }, body: body === undefined ? undefined : JSON.stringify(body) })
  assert(r.ok, `${method} ${path}: ${r.status}`)
  return r.status === 204 ? null : r.json()
}
async function until(check, label) {
  const deadline = Date.now() + 20000
  while (Date.now() < deadline) {
    if (await check()) return
    await new Promise(r => setTimeout(r, 100))
  }
  throw Error(`Timed out: ${label}`)
}
const session = await api('POST', '/auth/login', { username: 'nicholas', password: credentials.users?.nicholas || credentials.password })
const browser = await chromium.launch({ executablePath: '/usr/bin/chromium', headless: true, args: ['--no-sandbox'] })
let terminalId
try {
  await until(async () => (await api('GET', '/hosts', undefined, session.token)).some(h => h.name === hostName && h.online), 'host connected')
  const dm = await api('POST', '/dms', { member_ids: [session.user.id] }, session.token)
  const context = await browser.newContext({ viewport: { width: 1440, height: 900 } })
  await context.addCookies([{ name: 'den_session', value: session.token, url: base, httpOnly: true, secure: base.startsWith('https:'), sameSite: 'Strict' }])
  await context.addInitScript(csrf => localStorage.setItem('den.csrf', csrf), session.csrf_token)
  const page = await context.newPage()
  page.setDefaultTimeout(20000)
  const errors = []
  page.on('pageerror', e => errors.push(e.stack || e.message))
  await page.goto(`${base}/c/${dm.id}`)
  const composer = page.getByRole('textbox', { name: 'Message Just you', exact: true })
  await composer.fill(`/terminal ${hostName}`)
  await composer.press('Enter')
  const editor = page.getByTestId('terminal-editor')
  await editor.waitFor()
  await until(() => editor.evaluate(el => !!el.denTerminal), 'Ghostty mounted')
  const messages = await api('GET', `/channels/${dm.id}/messages`, undefined, session.token)
  terminalId = messages.flatMap(m => m.objects || []).filter(o => o.kind === 'terminal').at(-1).id
  const text = () => editor.evaluate(el => {
    const b = el.denTerminal.buffer.active
    return Array.from({ length: b.length }, (_, i) => b.getLine(i)?.translateToString(true) || '').join('\n')
  })
  await editor.evaluate(el => el.denTerminal.focus())
  // Do not send the literal expected output, so local input echo cannot pass.
  const command = hostName === 'nicholas' ? "Write-Output ('M8_' + $env:USERNAME + '_OK')" : "printf 'M8_%s_OK\\n' \"$(id -un)\""
  await page.keyboard.type(command)
  await page.keyboard.press('Enter')
  const expected = process.env.DEN_SMOKE_EXPECT || { 'm8-debian': 'm8', 'Nicholas-Work.local': 'nicholascaron', nicholas: 'caron' }[hostName]
  assert(expected, 'Set DEN_SMOKE_EXPECT to the remote username')
  await until(async () => (await text()).split('\n').some(line => line.trim() === `M8_${expected}_OK`), 'remote PTY command output')
  const shots = process.env.DEN_SMOKE_SHOTS || '/mnt/storage/den-m8-audit'
  await mkdir(shots, { recursive: true })
  await page.screenshot({ path: `${shots}/terminal-${hostName}.png` })
  assert.deepEqual(errors, [])
  console.log(`PASS ${hostName}: browser keyboard -> remote PTY -> Ghostty; session ${terminalId}`)
} finally {
  if (terminalId) await api('DELETE', `/sessions/${terminalId}`, undefined, session.token)
  await api('POST', '/auth/logout', {}, session.token).catch(() => {})
  await browser.close()
}
