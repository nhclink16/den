// Run against a launched AppImage with Chromium's generated camera/microphone devices.
// The credential file and server are private test fixtures, never committed.
import { chromium } from 'playwright-core'
import { readFile, writeFile } from 'node:fs/promises'
import assert from 'node:assert/strict'
const base = process.env.DEN_SMOKE_URL || 'http://127.0.0.1:17010'
const credentials = JSON.parse(await readFile(process.env.DEN_SMOKE_CREDENTIALS || '/mnt/storage/den-electron-acceptance/credentials.json', 'utf8'))
const shots = 'docs/shots/pr/desktop-electron'
const app = await chromium.connectOverCDP(process.env.DEN_ELECTRON_CDP || 'http://127.0.0.1:19226')
const page = app.contexts()[0].pages()[0]
const errors = []
page.on('pageerror', e => errors.push(e.message))
function trackPeers() {
  window.__denTestPeers = []
  const Original = window.RTCPeerConnection
  window.RTCPeerConnection = class extends Original { constructor(...args) { super(...args); window.__denTestPeers.push(this) } }
}
await page.evaluate(trackPeers)
const peerBrowser = await chromium.launch({ executablePath: '/usr/bin/chromium', headless: true, args: ['--no-sandbox', '--use-fake-device-for-media-stream', '--use-fake-ui-for-media-stream', '--autoplay-policy=no-user-gesture-required'] })
const context = await peerBrowser.newContext({ viewport: { width: 1200, height: 772 }, permissions: ['camera', 'microphone'] })
await context.addInitScript(trackPeers)
const peer = await context.newPage()
const until = async (test, label) => { for(let n=0;n<150;n++) { if(await test()) return; await new Promise(r=>setTimeout(r,200)) }; throw Error(`Timed out: ${label}`) }
const join = async p => { await p.locator('nav.side').getByRole('button', { name: 'Join hangout', exact: true }).click(); await p.getByTestId('call-dock').waitFor({ timeout: 30000 }); await until(async () => await p.getByRole('button', { name: /^Turn camera (on|off)$/ }).isEnabled(), 'call connected'); const cameraOn = p.getByRole('button', { name: 'Turn camera on', exact: true }); if (await cameraOn.count()) await cameraOn.click() }
const stats = p => p.evaluate(async () => (await Promise.all(window.__denTestPeers.map(pc => pc.getStats()))).flatMap(r => [...r.values()].filter(s => ['inbound-rtp', 'outbound-rtp'].includes(s.type)).map(s => ({ type: s.type, kind: s.kind, bytesReceived: s.bytesReceived, bytesSent: s.bytesSent, framesDecoded: s.framesDecoded, totalAudioEnergy: s.totalAudioEnergy }))))
try {
  await peer.goto(base)
  await peer.getByLabel('Username', { exact: true }).fill('media_peer'); await peer.getByLabel('Password', { exact: true }).fill(credentials.password); await peer.getByRole('button', { name: 'Come in', exact: true }).click()
  await peer.locator('nav.side').waitFor()
  await join(page); await join(peer)
  await until(async () => (await stats(page)).some(s => s.kind === 'video' && s.framesDecoded > 10), 'AppImage decodes remote video')
  await until(async () => (await stats(peer)).some(s => s.kind === 'video' && s.framesDecoded > 10), 'peer decodes AppImage video')
  await until(async () => (await stats(page)).some(s => s.kind === 'audio' && s.bytesReceived > 1000 && s.totalAudioEnergy > 0), 'AppImage receives non-silent audio')
  await until(async () => (await stats(peer)).some(s => s.kind === 'audio' && s.bytesReceived > 1000 && s.totalAudioEnergy > 0), 'peer receives non-silent AppImage audio')
  const report = { app: await stats(page), peer: await stats(peer), audioElements: await page.locator('audio').evaluateAll(elements => elements.map(e => ({ paused: e.paused, readyState: e.readyState, muted: e.muted }))), renderer: await page.evaluate(() => ({ userAgent: navigator.userAgent, bridge: !!window.denDesktop, node: typeof window.require, size: [innerWidth, innerHeight] })), errors }
  assert.equal(report.renderer.bridge, true); assert.equal(report.renderer.node, 'undefined'); assert.deepEqual(errors, [])
  assert(report.audioElements.some(a => !a.paused && !a.muted && a.readyState >= 2))
  await page.screenshot({ path: `${shots}/linux-after.png` })
  await writeFile(`${shots}/linux-call-stats.json`, JSON.stringify(report, null, 2) + '\n')
  console.log('PASS AppImage: two-way decoded video, non-silent received audio in both directions, active audio playback; isolated renderer')
  // Leave both streams up long enough for a silent ffmpeg recording of the real app window.
  await new Promise(resolve => setTimeout(resolve, Number(process.env.DEN_CAPTURE_WAIT || 20000)))
} finally { await peerBrowser.close(); await app.close() }
