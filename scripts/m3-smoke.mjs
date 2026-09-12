import { chromium } from 'playwright-core'
import { readFile, mkdir } from 'node:fs/promises'
import assert from 'node:assert/strict'

const base = process.env.DEN_SMOKE_URL || 'http://localhost:5173'
const password = process.env.DEN_SMOKE_PASSWORD || JSON.parse(await readFile(process.env.DEN_SMOKE_CREDENTIALS || `${process.env.HOME}/.local/share/den-dev/credentials.json`, 'utf8')).password
const shots = new URL('../docs/shots/', import.meta.url)
await mkdir(shots, { recursive: true })
const browser = await chromium.launch({ executablePath: '/usr/bin/chromium', headless: true, args: [
  '--no-sandbox', '--use-fake-ui-for-media-stream', '--use-fake-device-for-media-stream',
  '--autoplay-policy=no-user-gesture-required', '--enable-usermedia-screen-capturing',
  '--auto-select-desktop-capture-source=Entire screen',
] })
const errors = []
const pages = []
async function until(check, label, timeout = 15000) {
  const end = Date.now() + timeout
  while (Date.now() < end) { if (await check()) return; await new Promise((r) => setTimeout(r, 100)) }
  throw new Error(`Timed out: ${label}`)
}
async function login(username) {
  const context = await browser.newContext({ viewport: { width: 1440, height: 900 }, permissions: ['microphone', 'camera'] })
  await context.addInitScript(() => {
    window.__m3pcs = []
    const Original = window.RTCPeerConnection
    window.RTCPeerConnection = class extends Original {
      constructor(...args) { super(...args); window.__m3pcs.push(this) }
    }
  })
  const page = await context.newPage(); pages.push(page)
  page.on('pageerror', (err) => errors.push(err.message))
  await page.goto(base)
  await page.getByLabel('Username', { exact: true }).fill(username)
  await page.getByLabel('Password', { exact: true }).fill(password)
  await page.getByRole('button', { name: 'Come in', exact: true }).click()
  await page.locator('nav.side').getByRole('button', { name: 'Join hangout', exact: true }).waitFor()
  // Stay in a text channel so the docked strip leaves the composer usable.
  await page.locator('nav.side a.row').filter({ hasText: 'general' }).click()
  return page
}
const tiles = (page) => page.getByTestId('call-tile')
const remote = (page, name) => tiles(page).filter({ hasText: name })
async function join(page) {
  await page.locator('nav.side').getByRole('button', { name: 'Join hangout', exact: true }).click()
  await page.getByTestId('call-dock').waitFor()
  await until(async () => !(await page.getByTitle('Camera (V)', { exact: true }).isDisabled()), 'join finished')
}
async function shot(page, name) { await page.screenshot({ path: new URL(`m3-${name}.png`, shots).pathname }) }
try {
  const a = await login('nicholas'), b = await login('bob')
  await join(a); await join(b)
  await until(async () => await tiles(a).count() === 2 && await tiles(b).count() === 2, 'two participants in both browsers')
  assert.equal(await remote(a, 'bob').count(), 1)
  assert.equal(await remote(b, 'nicholas').count(), 1)
  await a.getByRole('button', { name: 'Mute microphone', exact: true }).click()
  await until(async () => await remote(b, 'nicholas').getByTestId('muted-mic').count() === 1, 'remote mute indication', 2000)
  await a.getByRole('button', { name: 'Unmute microphone', exact: true }).click()
  await a.getByRole('button', { name: 'Turn camera on', exact: true }).click()
  await b.getByRole('button', { name: 'Turn camera on', exact: true }).click()
  await until(async () => await a.locator('[data-testid="call-tile"] video').count() === 2 && await b.locator('[data-testid="call-tile"] video').count() === 2, 'both cameras subscribed')
  await until(async () => a.locator('[data-testid="call-tile"] video').evaluateAll((vs) => vs.every((v) => v.videoWidth > 0 && v.readyState >= 2)), 'decoded camera frames')
  await shot(a, 'strip-desktop')
  await shot(a, 'dock-desktop')
  await a.getByRole('button', { name: 'Expand call', exact: true }).click(); await shot(a, 'grid-desktop')
  await a.getByRole('button', { name: 'Collapse call', exact: true }).click()
  // Third browser, real screen capture, PTT, and text during the call.
  const c = await login('ari'); await join(c)
  await c.getByRole('button', { name: 'Turn camera on', exact: true }).click()
  await until(async () => await tiles(a).count() === 3 && await tiles(b).count() === 3 && await tiles(c).count() === 3, 'three participants')
  await until(async () => a.locator('[data-testid="call-tile"] video').evaluateAll((vs) => vs.length === 3 && vs.every((v) => v.videoWidth > 0)), 'three decoded cameras')
  await a.getByRole('button', { name: 'Share screen', exact: true }).click()
  await until(async () => await b.getByTestId('screen-tile').count() === 1, 'remote screen share')
  await until(async () => b.getByTestId('screen-tile').locator('video').evaluate((v) => v.videoWidth > 0), 'decoded screen share')
  await until(async () => b.evaluate(async () => {
    const reports = await Promise.all(window.__m3pcs.filter((pc) => pc.connectionState === 'connected').map((pc) => pc.getStats()))
    return reports.some((r) => [...r.values()].some((s) => s.type === 'inbound-rtp' && s.kind === 'audio' && s.bytesReceived > 0))
  }), 'received remote audio packets')
  const mediaAddresses = await b.evaluate(async () => {
    const reports = await Promise.all(window.__m3pcs.filter((pc) => pc.connectionState === 'connected').map((pc) => pc.getStats()))
    return reports.flatMap((r) => [...r.values()].filter((s) => s.type === 'transport' && s.selectedCandidatePairId).map((s) => r.get(r.get(s.selectedCandidatePairId).remoteCandidateId)?.address))
  })
  assert(mediaAddresses.includes('100.116.27.23'), 'media uses the tailnet node IP')
  await until(async () => b.locator('body > audio').evaluateAll((els) => els.length >= 2 && els.every((el) => !el.paused && el.readyState >= 2)), 'remote audio playback')
  const screenBox = await a.getByTestId('screen-tile').boundingBox(), stripBox = await a.getByTestId('call-strip').boundingBox()
  assert(screenBox.y >= stripBox.y && screenBox.y + screenBox.height <= stripBox.y + stripBox.height, 'screen tile and label fit in strip')
  await shot(a, 'screen-desktop')
  await a.getByRole('button', { name: 'Stop sharing screen', exact: true }).click()
  await a.locator('textarea').fill('M3 smoke: chat stays usable during a call.')
  await a.locator('textarea').press('Enter')
  await b.getByText('M3 smoke: chat stays usable during a call.', { exact: true }).last().waitFor()
  // SPA navigation keeps the room and remote audio alive.
  await a.locator('a[title="Settings"]').click()
  await a.getByRole('link', { name: 'Voice', exact: true }).click()
  await until(async () => (await a.getByRole('meter', { name: 'Microphone level' }).getAttribute('aria-valuenow')) !== '0', 'microphone meter')
  await a.getByRole('radio', { name: 'Push to talk', exact: true }).check()
  await until(async () => await remote(b, 'nicholas').getByTestId('muted-mic').count() === 1, 'PTT begins muted', 2000)
  await a.locator('h2').click()
  await a.keyboard.down('Backquote')
  await until(async () => await remote(b, 'nicholas').getByTestId('muted-mic').count() === 0, 'PTT transmits', 2000)
  await a.keyboard.up('Backquote')
  await until(async () => await remote(b, 'nicholas').getByTestId('muted-mic').count() === 1, 'PTT releases', 2000)
  await a.getByLabel('Push-to-talk key', { exact: true }).focus()
  await a.keyboard.press('KeyT')
  await a.locator('h2').click(); await a.keyboard.down('KeyT')
  await until(async () => await remote(b, 'nicholas').getByTestId('muted-mic').count() === 0, 'custom PTT key', 2000)
  await a.evaluate(() => window.dispatchEvent(new Event('blur')))
  await until(async () => await remote(b, 'nicholas').getByTestId('muted-mic').count() === 1, 'PTT releases on blur', 2000)
  await a.keyboard.up('KeyT')
  await shot(a, 'settings-desktop')
  await a.getByRole('radio', { name: 'Voice activity', exact: true }).check()
  await a.locator('nav.side a.row').filter({ hasText: 'general' }).click()
  await a.locator('textarea').focus(); await a.keyboard.type('mvs')
  assert.equal(await a.getByRole('button', { name: 'Mute microphone', exact: true }).count(), 1, 'typing M does not mute')
  assert.equal(await a.getByRole('button', { name: 'Turn camera off', exact: true }).count(), 1, 'typing V does not disable camera')
  await a.locator('textarea').fill('')
  await a.locator('h1').click(); await a.keyboard.press('m')
  await until(async () => await remote(b, 'nicholas').getByTestId('muted-mic').count() === 1, 'M shortcut', 2000)
  await a.keyboard.press('m')
  // Switching into a DM creates a banner for its other member and keeps the call private.
  await a.getByTitle('Message bob', { exact: true }).click()
  await b.getByTitle('Message nicholas', { exact: true }).click()
  await a.getByRole('button', { name: 'Call', exact: true }).click()
  await b.getByRole('button', { name: 'nicholas is in a call · Join', exact: true }).waitFor()
  const dmId = new URL(a.url()).pathname.split('/').at(-1)
  assert.equal((await c.evaluate(async () => (await fetch('/calls')).json())).some((r) => r.channel_id === dmId), false)
  await b.getByRole('button', { name: 'nicholas is in a call · Join', exact: true }).click()
  await until(async () => await tiles(a).count() === 2 && await tiles(b).count() === 2 && await tiles(c).count() === 1, 'DM room switch')
  await shot(a, 'dm-desktop')
  await a.getByLabel('Join hangout', { exact: true }).click()
  await b.getByLabel('Join hangout', { exact: true }).click()
  await until(async () => await tiles(a).count() === 3 && await tiles(b).count() === 3, 'switch back to hangout')
  await a.locator('nav.side a.row').filter({ hasText: 'general' }).click()
  await b.locator('nav.side a.row').filter({ hasText: 'general' }).click()
  await c.reload()
  await c.getByLabel('Join hangout', { exact: true }).waitFor()
  await until(async () => await c.locator('.avatars > span').count() === 2, 'resync restores active room avatars')
  assert.equal(await c.getByTestId('call-dock').count(), 0, 'reload does not auto-join')
  await join(c)
  await until(async () => c.locator('[data-testid="call-tile"] video').evaluateAll((vs) => vs.length === 3 && vs.every((v) => v.videoWidth > 0)), 'camera preference survives reload')
  await a.setViewportSize({ width: 390, height: 844 })
  await shot(a, 'strip-mobile')
  await shot(a, 'dock-mobile')
  await a.getByRole('button', { name: 'Expand call', exact: true }).click(); await shot(a, 'grid-mobile')
  await a.getByRole('button', { name: 'Collapse call', exact: true }).click()
  await b.getByRole('button', { name: 'Leave call', exact: true }).click()
  await until(async () => await tiles(a).count() === 2 && await remote(a, 'bob').count() === 0, 'leaving removes the tile')
  await c.getByRole('button', { name: 'Leave call', exact: true }).click()
  await a.getByRole('button', { name: 'Leave call', exact: true }).click()
  await until(async () => (await a.evaluate(async () => (await fetch('/calls')).json())).every((r) => r.participant_ids.length === 0), 'empty call snapshot')
  assert.deepEqual(errors, [], 'browser errors')
  console.log('M3 smoke passed: two-way membership/mute/leave, three cameras, screen/audio delivery, PTT/custom key/blur, live text/shortcuts, DM privacy/switching, resync, tailnet media, desktop/mobile screenshots.')
} catch (err) {
  for (const [i, page] of pages.entries()) await page.screenshot({ path: new URL(`m3-failure-${i}.png`, shots).pathname }).catch(() => {})
  console.error(err.message)
  for (const page of pages) {
    const alert = await page.locator('[role="alert"]').allTextContents().catch(() => [])
    if (alert.length) console.error('UI error:', alert.join('; '))
  }
  process.exitCode = 1
} finally { await browser.close() }
