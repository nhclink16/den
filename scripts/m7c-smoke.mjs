import { chromium } from 'playwright-core'
import { readFile, mkdir } from 'node:fs/promises'
import assert from 'node:assert/strict'

const base = process.env.DEN_SMOKE_URL || 'http://localhost:7001'
const credentials = JSON.parse(await readFile(process.env.DEN_SMOKE_CREDENTIALS || `${process.env.HOME}/.local/share/den-dev/credentials.json`, 'utf8'))
const users = JSON.parse(process.env.DEN_SMOKE_USERS || '["nicholas","bob","ari"]')
const shots = new URL(process.env.DEN_SMOKE_SHOTS || '../docs/shots/', import.meta.url)
await mkdir(shots, { recursive: true })
const browser = await chromium.launch({ executablePath: '/usr/bin/chromium', headless: !process.env.DEN_SMOKE_HEADED, args: [
  '--no-sandbox', '--use-fake-ui-for-media-stream', '--use-fake-device-for-media-stream',
  '--autoplay-policy=no-user-gesture-required', '--enable-usermedia-screen-capturing', '--auto-select-desktop-capture-source=Entire screen',
] })
const pages = [], errors = []
async function until(check, label, timeout = 20000) {
  const end = Date.now() + timeout
  while (Date.now() < end) { if (await check()) return; await new Promise(r => setTimeout(r, 100)) }
  throw new Error(`Timed out: ${label}`)
}
async function login(user, mobile = false) {
  const context = await browser.newContext({ viewport: mobile ? { width: 390, height: 844 } : { width: 1440, height: 900 }, permissions: ['microphone', 'camera'] })
  await context.addInitScript(() => {
    localStorage.setItem('den.voice', JSON.stringify({ sounds: false, cameraOn: true, micOn: true }))
    window.__m7cpcs = []
    const Original = window.RTCPeerConnection
    window.RTCPeerConnection = class extends Original { constructor(...args) { super(...args); window.__m7cpcs.push(this) } }
    // Preserve the real capture implementation; only name the two fake devices
    // distinctly, since Chromium gives both the same default "screen:0" label.
    const capture = navigator.mediaDevices.getDisplayMedia.bind(navigator.mediaDevices)
    let captures = 0
    navigator.mediaDevices.getDisplayMedia = async (...args) => {
      const stream = await capture(...args)
      Object.defineProperty(stream.getVideoTracks()[0], 'label', { value: ++captures % 2 === 1 ? 'Helium · docs' : 'Editor · source' })
      return stream
    }
  })
  const page = await context.newPage(); pages.push(page)
  page.on('pageerror', e => errors.push(e.message))
  await page.goto(base)
  await page.getByLabel('Username', { exact: true }).fill(user)
  await page.getByLabel('Password', { exact: true }).fill(credentials.users?.[user] || credentials.password)
  await page.getByRole('button', { name: 'Come in', exact: true }).click()
  await page.getByLabel('Join hangout', { exact: true }).waitFor()
  return page
}
const grid = p => p.getByTestId('call-grid')
const tiles = p => grid(p).locator('.layout-tile')
const screens = p => tiles(p).filter({ has: p.getByTestId('screen-tile') })
const cams = p => tiles(p).filter({ has: p.getByTestId('call-tile') })
const stored = p => p.evaluate(() => JSON.parse(localStorage.getItem(Object.keys(localStorage).find(k => k.startsWith('den.call-layout:'))) || 'null'))
async function join(p) {
  if (!await p.getByLabel('Join hangout', { exact: true }).isVisible()) await p.getByLabel('Menu', { exact: true }).click()
  await p.getByLabel('Join hangout', { exact: true }).click()
  if (await p.getByLabel('Close menu', { exact: true }).isVisible()) await p.getByLabel('Close menu', { exact: true }).click({ position: { x: p.viewportSize().width - 8, y: 40 } })
  await p.getByTestId('call-dock').waitFor()
  await until(async () => !await p.getByTitle('Camera (V)', { exact: true }).first().isDisabled(), 'join complete')
  await p.getByLabel('Expand call', { exact: true }).click()
}
async function shot(p, name) {
  await until(async () => grid(p).locator('video').evaluateAll(vs => vs.every(v => v.videoWidth > 0 && v.readyState >= 2)), 'decoded frames before screenshot')
  await p.screenshot({ path: new URL(`m7c-${name}.png`, shots).pathname, animations: 'disabled' })
}
async function cell(t) { return t.evaluate(e => ({ col: +e.dataset.col, row: +e.dataset.row, w: +e.dataset.w, h: +e.dataset.h })) }
async function drag(p, locator, dx, dy) {
  const box = await locator.boundingBox(); assert(box)
  await p.mouse.move(box.x + box.width / 2, box.y + box.height / 2)
  await p.mouse.down(); await p.mouse.move(box.x + box.width / 2 + dx, box.y + box.height / 2 + dy, { steps: 12 }); await p.mouse.up()
}
try {
  const a = await login(users[0]), b = await login(users[1]), c = await login(users[2])
  const active = await a.evaluate(async () => (await (await fetch('/calls')).json()).filter(r => r.participant_ids.length))
  assert.deepEqual(active, [], 'hangout must be empty before this smoke')
  await join(a); await join(b); await join(c)
  await until(async () => await cams(a).count() === 3 && await cams(b).count() === 3, 'three participants')
  await a.getByLabel('Share screen', { exact: true }).first().click()
  await b.getByLabel('Share screen', { exact: true }).first().click()
  await until(async () => await screens(a).count() === 2 && await screens(c).count() === 2, 'two shares')
  await until(async () => grid(c).locator('video').evaluateAll(vs => vs.length === 5 && vs.every(v => v.videoWidth > 0)), 'five decoded streams')
  const shareBoxes = await Promise.all([screens(a).nth(0).boundingBox(), screens(a).nth(1).boundingBox()])
  assert(shareBoxes.every(s => s.width > 350 && s.height > 180), 'both shares large')
  assert(Math.abs(shareBoxes[0].width - shareBoxes[1].width) < 2, 'shares equal size')
  assert.equal((await cell(screens(a).nth(0))).row, (await cell(screens(a).nth(1))).row, 'shares side by side')
  await shot(a, 'auto-desktop')
  const cam = cams(a).last(), camKey = await cam.getAttribute('data-tile-key'), before = await cell(cam)
  const step = await grid(a).locator('.grid-canvas').evaluate(e => parseFloat(e.style.getPropertyValue('--step-x')))
  await drag(a, cam.locator('.label'), -step * 2, 0)
  await until(async () => (await stored(a))?.preset === 'Custom', 'drag selects Custom')
  const after = await cell(cam); assert.notEqual(after.col, before.col)
  assert.equal((await stored(a)).tiles[camKey].col, after.col)
  const share = screens(a).first(), shareKey = await share.getAttribute('data-tile-key'), oldSize = await cell(share)
  await share.hover(); await drag(a, share.getByLabel('Resize', { exact: true }), -step, -30)
  await until(async () => (await cell(share)).w < oldSize.w, 'corner resize')
  assert.equal((await stored(a)).tiles[shareKey].w, (await cell(share)).w)
  await shot(a, 'custom-desktop')
  // A keyboard move and resize use the same persisted packing pass.
  await cam.focus(); await a.keyboard.press('ArrowDown'); await a.keyboard.press('Shift+ArrowUp')
  assert.equal((await stored(a)).tiles[camKey].h, after.h - 1)
  await a.keyboard.press('Escape'); assert.equal(await cam.evaluate(e => e === document.activeElement), false)
  const custom = await stored(a)
  await grid(a).getByLabel('Leave call', { exact: true }).click(); await join(a)
  await until(async () => await cams(a).count() === 3, 'rejoin restored participants')
  assert.deepEqual(await stored(a), custom, 'saved layout survives leave/rejoin')
  assert.deepEqual(await cell(tiles(a).locator(`:scope[data-tile-key="${camKey}"]`)), { col: custom.tiles[camKey].col, row: custom.tiles[camKey].row, w: custom.tiles[camKey].w, h: custom.tiles[camKey].h })
  await a.getByLabel('Share screen', { exact: true }).first().click()
  await until(async () => await screens(a).count() === 2, 'share restored after rejoin')
  assert.equal((await cell(grid(a).locator(`[data-tile-key="${shareKey}"]`))).w, custom.tiles[shareKey].w, 'returning share reuses its saved size')
  await a.setViewportSize({ width: 390, height: 844 })
  const mobileCustom = grid(a).locator(`[data-tile-key="${camKey}"]`)
  await mobileCustom.getByLabel('Pin', { exact: true }).click()
  assert.deepEqual((await stored(a)).tiles[camKey], { ...custom.tiles[camKey], pinned: true }, 'mobile pin preserves desktop Custom coordinates')
  await mobileCustom.getByLabel('Pin', { exact: true }).click()
  await a.setViewportSize({ width: 1440, height: 900 })
  await until(async () => (await cams(a).last().boundingBox()).width > 200, 'desktop layout settles after mobile')
  const focusCam = cams(a).last(), camWidth = (await focusCam.boundingBox()).width
  await focusCam.getByLabel('Pin', { exact: true }).click()
  await screens(a).first().getByLabel('Pin', { exact: true }).click()
  await grid(a).getByRole('button', { name: 'Focus', exact: true }).click()
  await until(async () => (await focusCam.boundingBox()).width > camWidth + 30, 'Focus grows pinned camera')
  assert.equal(await tiles(a).count(), 5)
  await shot(a, 'focus-desktop')
  // Real Document PiP (or the browser's real window.open fallback), no mocked API.
  const popKey = await screens(a).first().getAttribute('data-tile-key')
  const popTile = grid(a).locator(`[data-tile-key="${popKey}"]`)
  await popTile.locator('video').evaluate(v => { v.dataset.beforePop = 'same-element' })
  await popTile.getByLabel('Pop out', { exact: true }).click()
  await popTile.getByText('popped out · bring back', { exact: true }).waitFor()
  await shot(a, 'popped-desktop')
  const native = await a.evaluate(() => !!window.documentPictureInPicture?.window)
  if (await a.evaluate(() => 'documentPictureInPicture' in window)) assert(native, 'native Document PiP used when available')
  const floating = a.context().pages().find(p => p !== a)
  assert(floating, 'floating window opened')
  await until(async () => floating.locator('video').evaluate(v => v.videoWidth > 0 && !v.paused), 'popped video playing')
  assert.equal(await floating.locator('video').getAttribute('data-before-pop'), 'same-element')
  const time = await floating.locator('video').evaluate(v => v.currentTime)
  await until(async () => await floating.locator('video').evaluate(v => v.currentTime) > time, 'popped media advances')
  // The mounted component's controls still work in the other document.
  if (await floating.getByLabel('Share quality', { exact: true }).count()) {
    await floating.getByLabel('Share quality', { exact: true }).click(); await floating.getByRole('button', { name: 'Smooth', exact: true }).click()
  }
  await until(async () => a.evaluate(() => window.__m7cpcs.some(pc => pc.getSenders().some(s => s.track?.kind === 'video' && s.getParameters().degradationPreference === 'maintain-framerate' && s.getParameters().encodings.every(e => e.maxFramerate === 30)))), 'Smooth sender preference and 30 fps cap')
  await floating.close()
  await until(async () => await popTile.locator('video').count() === 1, 'closing popout brings tile back')
  assert.equal(await popTile.locator('video').getAttribute('data-before-pop'), 'same-element')
  // Explicitly cover the fallback in a browser which also supports Document PiP.
  await a.evaluate(() => Object.defineProperty(window, 'documentPictureInPicture', { value: undefined, configurable: true }))
  await popTile.focus(); await a.keyboard.press('o')
  await popTile.getByText('popped out · bring back', { exact: true }).waitFor()
  await popTile.getByText('popped out · bring back', { exact: true }).click()
  await until(async () => await popTile.locator('video').count() === 1, 'fallback bring back')
  await grid(a).getByLabel('Call options', { exact: true }).click()
  await grid(a).getByRole('button', { name: 'Reset layout', exact: true }).click()
  assert.equal(await stored(a), null)
  assert.equal(await grid(a).getByRole('button', { name: 'Auto', exact: true }).getAttribute('aria-pressed'), 'true')
  // Two captures from one person, stable publication names and individual stop.
  await grid(b).getByLabel('Stop sharing screen', { exact: true }).click()
  await a.getByLabel('Share another', { exact: true }).first().click()
  await until(async () => await screens(b).count() === 2 && await screens(b).getByText('Editor · source', { exact: false }).count() === 1, 'two labeled shares from one participant')
  assert.equal(new Set(await screens(b).locator('[data-connection-id]').evaluateAll(es => es.map(e => e.dataset.connectionId))).size, 1)
  await until(async () => grid(a).getByTestId('screen-tile').getByTestId('sent-quality').evaluateAll(es => es.length === 2 && es.every(e => /\d+p · \d+/.test(e.textContent))), 'actual local sender readouts')
  assert(await grid(b).getByTestId('connection-quality').count() >= 5, 'viewer quality bars')
  await shot(b, 'two-shares-one-person-desktop')
  await a.getByLabel('Share another', { exact: true }).first().click()
  await until(async () => await screens(b).count() === 3, 'three shares at cap')
  assert.equal(await a.getByLabel('Share another', { exact: true }).first().isDisabled(), true)
  assert.equal(await a.getByLabel('Share another', { exact: true }).first().getAttribute('title'), 'Up to 3 at once')
  await grid(a).getByLabel('Stop screen-1', { exact: true }).click()
  await until(async () => await screens(b).count() === 2 && await grid(b).locator('[data-share-name="screen-1"]').count() === 0, 'stop one preserves other shares')
  await until(async () => screens(b).locator('video').evaluateAll(vs => vs.length === 2 && vs.every(v => v.videoWidth > 0 && !v.paused)), 'remaining shares play')
  // Mobile uses a fresh local layout, no dragging/resizing or floating window.
  await grid(c).getByLabel('Leave call', { exact: true }).click()
  await c.setViewportSize({ width: 390, height: 844 }); await c.reload()
  await c.getByLabel('Menu', { exact: true }).waitFor(); await join(c)
  await until(async () => await tiles(c).count() === 5, 'mobile all streams')
  assert.equal(await grid(c).getByRole('button', { name: 'Focus', exact: true }).getAttribute('aria-pressed'), 'true')
  assert.equal(await grid(c).getByLabel('Resize', { exact: true }).count(), 0)
  assert.equal(await grid(c).getByLabel('Pop out', { exact: true }).count(), 0)
  await until(async () => (await screens(c).first().boundingBox()).width > 340 && (await cams(c).first().boundingBox()).width > 100, 'mobile tiles fill the viewport')
  await shot(c, 'focus-mobile')
  await cams(c).first().getByLabel('Pin', { exact: true }).click()
  assert.equal(await cams(c).first().getByLabel('Pin', { exact: true }).getAttribute('aria-pressed'), 'true')
  await grid(a).getByLabel('Stop sharing screen', { exact: true }).click()
  await until(async () => await screens(b).count() === 0, 'main toggle stops all')
  for (const p of [a, b, c]) await grid(p).getByLabel('Leave call', { exact: true }).click()
  assert.deepEqual(errors, [], 'browser errors')
  console.log(`M7c smoke passed (${base}; ${native ? 'native Document PiP' : 'window.open'}): three cameras, two large shares, drag/resize and keyboard persistence, multi-pin Focus, rejoin, pop/return/fallback, reset, three-share cap and individual stop, sender stats/viewer bars, mobile Focus.`)
} catch (err) {
  for (const [i, p] of pages.entries()) await p.screenshot({ path: new URL(`m7c-failure-${i}.png`, shots).pathname }).catch(() => {})
  console.error(err.stack)
  if (errors.length) console.error(errors)
  for (const p of pages) { const alerts = await p.locator('[role="alert"]').allTextContents().catch(() => []); if (alerts.length) console.error('UI error:', alerts.join('; ')) }
  process.exitCode = 1
} finally { await browser.close() }
