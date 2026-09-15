// Packaged Electron, an X11 window manager, and a private Den/LiveKit fixture.
// The native profile must already be signed into the private server. Never use a
// real member's profile: this smoke resets the private call's saved layout.
import { chromium } from 'playwright-core'
import { execFileSync, spawn } from 'node:child_process'
import { mkdir, readFile, writeFile } from 'node:fs/promises'
import assert from 'node:assert/strict'

const base = process.env.DEN_SMOKE_URL || 'http://127.0.0.1:17010'
const credentials = JSON.parse(await readFile(process.env.DEN_SMOKE_CREDENTIALS || '/mnt/storage/den-electron-acceptance/credentials.json', 'utf8'))
const output = process.env.DEN_SMOKE_SHOTS || 'docs/shots/pr/portrait-layout-memory'
const display = process.env.DISPLAY || ':101'
const x = (...args) => execFileSync('xdotool', args.map(String), { env: { ...process.env, DISPLAY: display } }).toString().trim()
const windowId = process.env.DEN_ELECTRON_WINDOW || x('search', '--onlyvisible', '--class', '^den$').split('\n')[0]
const app = await chromium.connectOverCDP(process.env.DEN_ELECTRON_CDP || 'http://127.0.0.1:19230')
const page = app.contexts()[0].pages()[0]
page.setDefaultTimeout(30000)
const errors = [], results = []
page.on('pageerror', error => errors.push(error.message))
let browser, peer, recording
const sleep = ms => new Promise(resolve => setTimeout(resolve, ms))
const until = async (fn, message) => { for (let i = 0; i < 150; i++) { if (await fn()) return; await sleep(200) } throw Error(message) }
const peers = () => {
  window.__portraitPeers = []
  const Original = window.RTCPeerConnection
  window.RTCPeerConnection = class extends Original { constructor(...args) { super(...args); window.__portraitPeers.push(this) } }
}
async function resize(width, height) {
  // Resize the native X11 client window. Playwright viewport emulation would not
  // prove that Electron responds to a monitor/window resize.
  x('windowsize', windowId, width, height); x('windowmove', windowId, 20, 20); x('windowactivate', windowId)
  await until(async () => await page.evaluate(([w, h]) => innerWidth === w && innerHeight === h, [width, height]), 'Native window did not reach the requested dimensions')
  await sleep(600)
}
async function capture(name) {
  const result = await page.evaluate(() => {
    const rect = e => { const r = e.getBoundingClientRect(); return { x: r.x, y: r.y, width: r.width, height: r.height } }
    return {
      viewport: [innerWidth, innerHeight], preset: document.querySelector('.presets').innerText,
      activePreset: document.querySelector('.presets button[aria-pressed="true"]')?.textContent.trim() || 'Custom',
      savedLayouts: Object.fromEntries(Object.keys(localStorage).filter(k => k.startsWith('den.call-layout:')).map(k => [k, localStorage.getItem(k)])),
      scroll: rect(document.querySelector('.grid-scroll')), canvas: rect(document.querySelector('.grid-canvas')),
      tiles: [...document.querySelectorAll('.layout-tile')].map(e => ({
        key: e.dataset.tileKey, kind: e.dataset.kind,
        cell: Object.fromEntries(['col', 'row', 'w', 'h'].map(k => [k, Number(e.dataset[k])])), rect: rect(e),
        video: [...e.querySelectorAll('video')].map(v => ({ width: v.videoWidth, height: v.videoHeight, frames: v.getVideoPlaybackQuality().totalVideoFrames, readyState: v.readyState })),
      })),
    }
  })
  assert.equal(result.tiles.filter(t => t.kind === 'screen').length, 1)
  assert.equal(result.tiles.filter(t => t.kind === 'cam').length, 2)
  for (const t of result.tiles) assert(t.video.some(v => v.frames > 10 && v.readyState === 4), `${name}: ${t.kind} has no decoded video`)
  for (const [i, a] of result.tiles.entries()) for (const b of result.tiles.slice(i + 1)) {
    const intersects = a.rect.x < b.rect.x + b.rect.width - 1 && a.rect.x + a.rect.width > b.rect.x + 1 && a.rect.y < b.rect.y + b.rect.height - 1 && a.rect.y + a.rect.height > b.rect.y + 1
    assert(!intersects, `${name}: tiles overlap`)
  }
  await page.screenshot({ path: `${output}/${name}.png` })
  results.push({ name, ...result }); return result
}
const shape = r => r.tiles.map(t => ({ key: t.key, cell: t.cell, rect: t.rect }))
try {
  await mkdir(output, { recursive: true })
  await page.evaluate(origin => { localStorage.setItem('den.native.origin', origin); localStorage.setItem('den.voice', JSON.stringify({ mode: 'activity', cameraOn: true, micOn: true, sounds: false })) }, base)
  await page.reload(); await page.locator('nav.side').waitFor()
  await page.locator('nav.side a').filter({ hasText: 'general' }).click()
  await page.evaluate(peers)
  browser = await chromium.launch({ executablePath: '/usr/bin/chromium', headless: false, env: { ...process.env, DISPLAY: display }, args: ['--no-sandbox', '--use-fake-device-for-media-stream', '--use-fake-ui-for-media-stream', '--autoplay-policy=no-user-gesture-required', '--enable-usermedia-screen-capturing', '--auto-select-desktop-capture-source=Entire screen'] })
  const context = await browser.newContext({ viewport: { width: 1280, height: 800 }, permissions: ['camera', 'microphone'] })
  await context.addInitScript(peers)
  await context.addInitScript(() => localStorage.setItem('den.voice', JSON.stringify({ mode: 'activity', cameraOn: true, micOn: true, sounds: false })))
  peer = await context.newPage(); peer.on('pageerror', error => errors.push(error.message))
  await peer.goto(base)
  await peer.getByLabel('Username', { exact: true }).fill('media_peer'); await peer.getByLabel('Password', { exact: true }).fill(credentials.password)
  await peer.getByRole('button', { name: 'Come in', exact: true }).click(); await peer.locator('nav.side').waitFor()
  for (const p of [page, peer]) {
    await p.getByRole('button', { name: 'Join hangout', exact: true }).click(); await p.getByTestId('call-dock').waitFor()
    await p.getByRole('button', { name: 'Turn camera off', exact: true }).waitFor()
  }
  await peer.getByRole('button', { name: 'Share screen', exact: true }).click()
  await page.getByTestId('screen-tile').waitFor()
  await page.getByRole('button', { name: 'Expand call', exact: true }).click()
  // R is the app's reset-layout command, clearing old Custom/pin geometry.
  await page.locator('.layout-tile').first().focus(); await page.keyboard.press('r')
  await until(async () => await page.locator('.layout-tile video').evaluateAll(v => v.length === 3 && v.every(e => e.getVideoPlaybackQuality().totalVideoFrames > 10)), 'Three live videos did not decode')
  // Keep the peer's fixture window out of the silent native screen recording.
  const peerWindow = x('search', '--onlyvisible', '--class', '^chromium$').split('\n').at(-1)
  x('windowminimize', peerWindow)
  await resize(1900, 1100)
  const wide = await capture('landscape'); await sleep(1800)
  await resize(1100, 1900); const tall = await capture('portrait'); await sleep(2500)
  assert(tall.tiles.every(t => t.cell.col === 0 && t.cell.w === 12), 'Auto did not stack full-width portrait tiles')
  assert(tall.canvas.height / tall.scroll.height > .6, 'Portrait layout still uses too little of the available height')
  const top = tall.canvas.y - tall.scroll.y, bottom = tall.scroll.y + tall.scroll.height - tall.canvas.y - tall.canvas.height
  assert(Math.abs(top - bottom) < 2, 'Portrait layout is not vertically centered')
  assert(tall.tiles.every(t => t.rect.y >= tall.scroll.y && t.rect.y + t.rect.height <= tall.scroll.y + tall.scroll.height), 'Portrait tiles leave the visible call area')
  await resize(1900, 1100); const back = await capture('landscape-return'); await sleep(1800)
  assert.deepEqual(shape(wide), shape(back), 'Landscape geometry changed after the round trip')
  for (const before of wide.tiles) assert(back.tiles.find(t => t.key === before.key).video[0].frames > before.video[0].frames, 'Video stalled during native resizing')
  // Reproduce Andy's half-width screenshot using real tile keyboard controls.
  const screen = page.locator('.layout-tile[data-kind="screen"]'), cams = page.locator('.layout-tile[data-kind="cam"]')
  await screen.focus(); for (let i = 0; i < 6; i++) await page.keyboard.press('Shift+ArrowLeft')
  for (let cam = 0; cam < 2; cam++) { await cams.nth(cam).focus(); for (let i = 0; i < 3; i++) await page.keyboard.press('Shift+ArrowLeft') }
  await cams.nth(1).focus(); for (let i = 0; i < 3; i++) await page.keyboard.press('ArrowLeft')
  await page.mouse.move(1890, 1080); await sleep(500)
  const custom = await capture('custom-half-width'); assert(custom.preset.includes('Custom')); assert.equal(custom.tiles[0].cell.w, 6)
  if (process.env.DEN_RECORD === '1') {
    recording = spawn('ffmpeg', ['-y', '-loglevel', 'error', '-f', 'x11grab', '-framerate', '12', '-video_size', '1940x1960', '-i', display, '-an', '-c:v', 'libx264', '-preset', 'ultrafast', '-threads', '2', '-pix_fmt', 'yuv420p', `${output}/custom-rotation.mp4`])
    await sleep(1500)
  }
  await resize(1100, 1900); const firstPortrait = await capture('custom-to-portrait-default'); await sleep(1800)
  assert.equal(firstPortrait.activePreset, 'Auto', 'First portrait visit inherited landscape Custom')
  assert.deepEqual(shape(tall), shape(firstPortrait), 'Portrait did not start from its full-width preset')
  assert(firstPortrait.canvas.height / firstPortrait.scroll.height > .6, 'Portrait stayed at the old 38% height baseline')
  assert.deepEqual(custom.savedLayouts, firstPortrait.savedLayouts, 'Rotation modified the saved landscape layout')
  await resize(1900, 1100); const customBack = await capture('custom-landscape-return'); await sleep(1800); assert.deepEqual(shape(custom), shape(customBack))
  assert.deepEqual(custom.savedLayouts, customBack.savedLayouts)
  // Author a different arrangement in portrait using the real resize control.
  await resize(1100, 1900); await screen.focus(); await page.keyboard.press('Shift+ArrowDown'); await sleep(500)
  const portraitCustom = await capture('portrait-custom'); assert.equal(portraitCustom.activePreset, 'Custom')
  const landscapeKey = Object.keys(custom.savedLayouts).find(k => !k.endsWith(':portrait'))
  assert(landscapeKey && portraitCustom.savedLayouts[`${landscapeKey}:portrait`], 'Portrait layout was not stored separately')
  assert.equal(custom.savedLayouts[landscapeKey], portraitCustom.savedLayouts[landscapeKey], 'Landscape layout bytes changed while editing portrait')
  await resize(1900, 1100); const separateWide = await capture('separate-landscape-custom'); assert.deepEqual(shape(custom), shape(separateWide))
  await resize(1100, 1900); const separateTall = await capture('portrait-custom-return'); assert.deepEqual(shape(portraitCustom), shape(separateTall))
  assert.deepEqual(portraitCustom.savedLayouts, separateTall.savedLayouts)
  assert.equal(custom.savedLayouts[landscapeKey], separateWide.savedLayouts[landscapeKey], 'Landscape layout did not return byte-identical')
  if (recording) { const done = new Promise(resolve => recording.once('exit', resolve)); recording.stdin.write('q'); await done; recording = null }
  // Diagnostic: a sidebar can cross the call-area orientation boundary while the
  // native window stays landscape. Capture that interaction without treating it
  // as a failure of the two independently stored arrangements.
  await resize(1400, 1100); const withPeople = await capture('sidebar-visible')
  await page.getByTitle('Toggle people (Ctrl+Shift+M)').click(); await sleep(700)
  const withoutPeople = await capture('sidebar-hidden')
  assert.deepEqual(withPeople.savedLayouts, withoutPeople.savedLayouts, 'Sidebar toggle modified saved layout bytes')
  await page.getByTitle('Toggle people (Ctrl+Shift+M)').click(); await sleep(700)
  await resize(1100, 1900)
  // Reset clears both orientations, including the one that is not visible.
  await screen.focus(); await page.keyboard.press('r'); await sleep(500)
  const reset = await capture('reset-both'); assert(!reset.savedLayouts[landscapeKey] && !reset.savedLayouts[`${landscapeKey}:portrait`])
  await resize(1900, 1100)
  await page.getByRole('button', { name: 'Auto', exact: true }).click(); await sleep(500)
  const restored = await capture('auto-restored'); assert.deepEqual(shape(wide), shape(restored))
  const connections = await page.evaluate(() => window.__portraitPeers.map(p => ({ connection: p.connectionState, ice: p.iceConnectionState })))
  assert(connections.length >= 2 && connections.every(p => p.connection === 'connected'))
  assert.deepEqual(errors, [])
  await writeFile(`${output}/verification.json`, JSON.stringify({ nativeWindowResized: true, generatedCameraAndDisplayInput: true, connections, errors, results }, null, 2) + '\n')
  console.log('PASS: live share + two cams; landscape/portrait/landscape; landscape Custom restored; portrait starts Auto and remembers its own Custom; reset clears both; decoded video continues')
} finally {
  if (recording) recording.stdin.write('q')
  for (const p of [page, peer].filter(Boolean)) if (!p.isClosed()) {
    const leave = p.getByRole('button', { name: 'Leave call', exact: true }).first()
    if (await leave.count()) await leave.click().catch(() => {})
  }
  await browser?.close(); await app.close()
}
