import { chromium } from 'playwright-core'
import { readFile, mkdir } from 'node:fs/promises'
import assert from 'node:assert/strict'

const scratch = process.argv[2]
const base = 'http://127.0.0.1:17400'
const { password } = JSON.parse(await readFile(`${scratch}/credentials.json`, 'utf8'))
async function api(path, body, token) {
  const response = await fetch(base + path, { method: body ? 'POST' : 'GET', headers: { 'Content-Type': 'application/json', ...(token ? { Authorization: `Bearer ${token}` } : {}) }, body: body ? JSON.stringify(body) : undefined })
  assert(response.ok, `API ${path}: ${response.status}`)
  return response.json()
}
const admin = await api('/auth/init', { username: 'nicholas', password, bootstrap_token: (await readFile(`${scratch}/bootstrap.key`, 'utf8')).trim() })
const invite = await api('/invites', { uses: 2, expires_in_hours: 1 }, admin.token)
for (const username of ['bob', 'ari']) await api('/auth/register', { username, password, invite: invite.code })
if (!(await api('/channels', undefined, admin.token)).some((c) => c.name === 'general')) await api('/channels', { name: 'general', position: 0 }, admin.token)
await mkdir(`${scratch}/shots`, { recursive: true })
const browser = await chromium.launch({ executablePath: '/usr/bin/chromium', headless: true, args: [
  '--no-sandbox', '--use-fake-ui-for-media-stream', '--use-fake-device-for-media-stream',
  '--autoplay-policy=no-user-gesture-required', '--enable-usermedia-screen-capturing',
  '--auto-select-desktop-capture-source=Entire screen',
] })
const pages = [], errors = []
async function until(check, label) {
  for (let i = 0; i < 150; i++) { if (await check()) return; await new Promise((r) => setTimeout(r, 100)) }
  throw new Error(`Timed out: ${label}`)
}
async function login(username) {
  const context = await browser.newContext({ viewport: { width: 1440, height: 900 }, permissions: ['microphone', 'camera'] })
  await context.addInitScript(() => {
    window.__pcs = []
    const Original = RTCPeerConnection
    window.RTCPeerConnection = class extends Original { constructor(...args) { super(...args); window.__pcs.push(this) } }
    // Chromium's fake full-screen capture lacks system audio on Linux. Add a
    // generated tone so the test exercises the screen-audio publication path.
    const capture = navigator.mediaDevices.getDisplayMedia.bind(navigator.mediaDevices)
    navigator.mediaDevices.getDisplayMedia = async (...args) => {
      const stream = await capture(...args)
      const audio = new AudioContext(), oscillator = audio.createOscillator(), destination = audio.createMediaStreamDestination()
      oscillator.connect(destination); oscillator.start(); await audio.resume()
      for (const track of destination.stream.getAudioTracks()) stream.addTrack(track)
      stream.getVideoTracks()[0].addEventListener('ended', () => { oscillator.stop(); void audio.close() })
      return stream
    }
  })
  const page = await context.newPage(); pages.push(page)
  page.on('pageerror', (e) => errors.push(e.message))
  await page.goto(base)
  await page.getByLabel('Username', { exact: true }).fill(username)
  await page.getByLabel('Password', { exact: true }).fill(password)
  await page.getByRole('button', { name: 'Come in', exact: true }).click()
  await page.getByRole('button', { name: 'Join hangout', exact: true }).click()
  await page.getByTestId('call-dock').waitFor()
  await until(async () => !await page.getByTitle('Camera (V)', { exact: true }).isDisabled(), 'join complete')
  await page.locator('nav.side a.row').filter({ hasText: 'general' }).click()
  return page
}
const tiles = (p) => p.getByTestId('call-tile')
const screens = (p) => p.getByTestId('screen-tile')
try {
  const desktop = await login('nicholas'), phone = await login('nicholas'), tablet = await login('nicholas'), observer = await login('bob')
  for (const page of pages) await until(async () => await tiles(page).count() === 4, 'four connections coexist')
  assert.equal(await desktop.getByRole('button', { name: 'Mute microphone', exact: true }).count(), 1)
  for (const page of [phone, tablet]) {
    assert.equal(await page.getByRole('button', { name: 'Unmute microphone', exact: true }).count(), 1)
    assert.equal(await page.getByRole('button', { name: 'Play sound on this device', exact: true }).count(), 1)
    assert(await page.locator('[role="status"]').getByText('Mic and sound are off on this device. Your other device stays connected.').count())
  }
  const ownTiles = tiles(observer).filter({ hasText: 'nicholas' })
  assert.equal(await ownTiles.count(), 3)
  const ids = await ownTiles.evaluateAll((els) => els.map((el) => el.dataset.connectionId))
  assert.equal(new Set(ids).size, 3)
  assert((await ownTiles.evaluateAll((els) => els.map((el) => el.dataset.userId))).every((id) => id === admin.user.id))
  const hangout = (await api('/channels', undefined, admin.token)).find((c) => c.name === 'hangout')
  const state = (await api('/calls', undefined, admin.token)).find((c) => c.channel_id === hangout.id)
  assert.equal(state.participant_ids.filter((id) => id === admin.user.id).length, 1)
  assert.equal(state.participant_ids.length, 2)
  // Rejoining this account on one browser must leave the other two connected.
  await phone.reload()
  await phone.getByRole('button', { name: 'Join hangout', exact: true }).waitFor()
  await until(async () => tiles(desktop).count().then((n) => n === 3), 'phone reload only removes phone')
  await phone.getByRole('button', { name: 'Join hangout', exact: true }).click()
  await until(async () => tiles(desktop).count().then((n) => n === 4), 'phone rejoins alongside desktop and tablet')
  await until(async () => !await phone.getByTitle('Camera (V)', { exact: true }).isDisabled(), 'phone rejoin complete')
  for (const page of [desktop, phone, tablet]) await page.getByRole('button', { name: 'Turn camera on', exact: true }).click()
  await until(async () => ownTiles.locator('video').evaluateAll((els) => els.length === 3 && els.every((el) => el.videoWidth > 0)), 'three same-account cameras decoded')
  for (const page of [desktop, phone, tablet]) await page.getByRole('button', { name: 'Share screen', exact: true }).click()
  await until(async () => screens(observer).evaluateAll((els) => els.length === 3 && els.every((el) => el.querySelector('video')?.videoWidth > 0)), 'three same-account screens decoded')
  await until(async () => desktop.locator('body > audio').count().then((n) => n === 3), 'desktop receives other-user mic and two own-device screen audio tracks')
  // Unmuting a second own-device mic must not add a self-echo audio element.
  await phone.getByRole('button', { name: 'Unmute microphone', exact: true }).click()
  await until(async () => ownTiles.getByTestId('muted-mic').count().then((n) => n === 1), 'second microphone published')
  assert.equal(await desktop.locator('body > audio').count(), 3)
  await phone.getByRole('button', { name: 'Mute microphone', exact: true }).click()
  assert.equal(await phone.locator('body > audio').count(), 0, 'quiet phone subscribes to no remote audio')
  await phone.getByRole('button', { name: 'Play sound on this device', exact: true }).click()
  await until(async () => phone.locator('body > audio').evaluateAll((els) => els.length === 3 && els.every((el) => !el.muted && !el.paused)), 'phone audio can be enabled independently')
  await phone.getByRole('button', { name: 'Mute sound on this device', exact: true }).click()
  assert.equal(await phone.locator('body > audio').count(), 0)
  await until(async () => desktop.evaluate(async () => {
    const stats = await Promise.all(window.__pcs.map((pc) => pc.getStats()))
    return stats.flatMap((r) => [...r.values()]).filter((s) => s.type === 'inbound-rtp' && s.kind === 'audio' && s.bytesReceived > 1000).length >= 3
  }), 'audio from both own-device shares received')
  await observer.getByRole('button', { name: 'Expand call', exact: true }).click()
  const boxes = await observer.locator('[data-testid="call-grid"] .tile').evaluateAll((els) => els.map((el) => {
    const { left, right, top, bottom } = el.getBoundingClientRect(); return { left, right, top, bottom }
  }))
  for (let i = 0; i < boxes.length; i++) for (let j = i + 1; j < boxes.length; j++) {
    const a = boxes[i], b = boxes[j]
    assert(a.right <= b.left || b.right <= a.left || a.bottom <= b.top || b.bottom <= a.top, 'expanded tiles never overlap')
  }
  assert(boxes.every((b) => b.bottom <= 900), 'all seven streams/participants fit the desktop grid')
  await observer.screenshot({ path: `${scratch}/shots/multidevice-desktop.png` })
  await phone.setViewportSize({ width: 390, height: 844 })
  await phone.screenshot({ path: `${scratch}/shots/multidevice-phone.png` })
  await phone.getByRole('button', { name: 'Leave call', exact: true }).click()
  await until(async () => tiles(desktop).count().then((n) => n === 3), 'leaving phone leaves desktop and tablet connected')
  assert.equal(await screens(observer).count(), 2)
  assert((await api('/calls', undefined, admin.token)).find((c) => c.channel_id === hangout.id).participant_ids.includes(admin.user.id))
  await tablet.getByRole('button', { name: 'Leave call', exact: true }).click()
  assert.equal(await desktop.getByRole('button', { name: 'Mute microphone', exact: true }).count(), 1)
  await desktop.getByRole('button', { name: 'Leave call', exact: true }).click()
  await until(async () => tiles(observer).count().then((n) => n === 1), 'last own connection leaves')
  assert(!(await api('/calls', undefined, admin.token)).find((c) => c.channel_id === hangout.id).participant_ids.includes(admin.user.id))
  await observer.getByTestId('call-dock').getByRole('button', { name: 'Leave call', exact: true }).click()
  assert.deepEqual(errors, [])
  console.log('Multi-device smoke passed: three connections on one account, independent cameras and screen shares, two shared-audio streams to desktop, no own-mic echo, quiet secondary devices, unique user presence, independent leave, and screenshots.')
} catch (err) {
  for (const [index, page] of pages.entries()) await page.screenshot({ path: `${scratch}/shots/failure-${index}.png` }).catch(() => {})
  throw err
} finally { await browser.close() }
