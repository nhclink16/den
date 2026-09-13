// A disposable local server with an admin session. Never run against production.
// DEN_SMOKE_SESSION=/private/session.json node scripts/m8-instance-smoke.mjs
import { chromium } from 'playwright-core'
import { readFile, mkdir } from 'node:fs/promises'
import { randomBytes } from 'node:crypto'
import assert from 'node:assert/strict'

const base = 'http://127.0.0.1:17800'
const session = JSON.parse(await readFile(process.env.DEN_SMOKE_SESSION, 'utf8'))
const shots = process.env.DEN_SMOKE_SHOTS || '/mnt/storage/den-m8-audit'
await mkdir(shots, { recursive: true })
async function api(method, path, body, token = session.token) {
  const response = await fetch(base + path, { method, headers: { 'content-type': 'application/json', authorization: `Bearer ${token}` }, body: body === undefined ? undefined : JSON.stringify(body) })
  assert(response.ok, `${method} ${path}: ${response.status}`)
  return response.json()
}
async function until(check) {
  for (let n = 0; n < 100; n++) {
    if (await check()) return
    await new Promise(r => setTimeout(r, 100))
  }
  throw Error('Timed out waiting for UI state')
}
const original = await api('GET', '/settings')
const browser = await chromium.launch({ executablePath: '/usr/bin/chromium', args: ['--no-sandbox'] })
try {
  const context = await browser.newContext({ viewport: { width: 1440, height: 900 } })
  await context.addCookies([{ name: 'den_session', value: session.token, url: base, httpOnly: true, sameSite: 'Strict' }])
  await context.addInitScript(csrf => {
    localStorage.setItem('den.csrf', csrf)
    window.sentNotifications = []
    window.Notification = class {
      static permission = 'granted'
      constructor(title, options) { window.sentNotifications.push({ title, options }) }
      close() {}
    }
  }, session.csrf_token)
  const page = await context.newPage()
  const second = await context.newPage()
  const errors = []
  page.on('pageerror', e => errors.push(e.message))
  await page.goto(base + '/settings/rooms')
  await second.goto(base + '/settings/rooms')
  await second.getByLabel('Server name', { exact: true }).waitFor()
  const field = page.getByLabel('Server name', { exact: true })
  await field.fill('Evening room 🌲')
  await field.press('Tab')
  assert.equal(await page.locator(':focus').innerText(), 'Save name')
  await page.keyboard.press('Enter')
  await page.getByRole('status').filter({ hasText: 'Server name saved.' }).waitFor()
  await until(async () => (await second.title()).endsWith('Evening room 🌲'))
  assert.equal(await second.getByLabel('Server name', { exact: true }).inputValue(), 'Evening room 🌲')
  assert.equal(await page.locator('.wordmark span').innerText(), 'Evening room 🌲')
  assert.equal(await page.locator('.wordmark svg').count(), 1)
  await page.screenshot({ path: `${shots}/m8-instance-settings.png` })

  const invite = await api('POST', '/invites', { uses: 1, expires_in_hours: 1 })
  const member = await api('POST', '/auth/register', { username: `m8_name_${randomBytes(3).toString('hex')}`, password: randomBytes(24).toString('hex'), invite: invite.code }, '')
  const channel = (await api('GET', '/channels')).find(c => c.kind === 'text')
  await api('POST', `/channels/${channel.id}/messages`, { content: `@${session.user.username} M8 instance notification check` }, member.token)
  await until(() => page.evaluate(() => window.sentNotifications.length > 0))
  assert((await page.evaluate(() => window.sentNotifications.at(-1).title)).startsWith('Evening room 🌲: '))

  const anonymous = await browser.newContext({ viewport: { width: 390, height: 844 } })
  const login = await anonymous.newPage()
  await login.goto(base + '/login')
  await login.getByRole('heading', { name: 'Evening room 🌲' }).waitFor()
  assert.equal(await login.title(), 'Evening room 🌲')
  await login.screenshot({ path: `${shots}/m8-instance-login.png` })
  await field.fill('W'.repeat(40))
  await page.getByRole('button', { name: 'Save name', exact: true }).click()
  await until(async () => (await second.title()).endsWith('W'.repeat(40)))
  await login.reload()
  await login.getByRole('heading', { name: 'W'.repeat(40) }).waitFor()
  assert(await login.evaluate(() => document.documentElement.scrollWidth <= innerWidth))
  await login.screenshot({ path: `${shots}/m8-instance-long-login.png` })
  await page.setViewportSize({ width: 390, height: 844 })
  await page.evaluate(() => new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve))))
  await page.screenshot({ path: `${shots}/m8-instance-mobile.png` })
  assert(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth))
  await field.fill('x'.repeat(41))
  assert(await page.getByRole('button', { name: 'Save name', exact: true }).isDisabled())
  assert.deepEqual(errors, [])
  console.log('Instance name passed: keyboard save, live second tab, sidebar mark, anonymous login/title, notification, 40-character mobile layout, 41-character rejection.')
} finally {
  await api('PUT', '/settings', original)
  await browser.close()
}
