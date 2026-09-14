// Foreground, disposable browser peer for native media tests. Ctrl-C closes it.
import { chromium } from 'playwright-core'
import { readFile } from 'node:fs/promises'
import { dirname, resolve } from 'node:path'
import assert from 'node:assert/strict'
const path = resolve(process.env.DEN_IOS_FIXTURE_CREDENTIALS || '')
assert.match(path, /^\/mnt\/storage\/den-ios-fixture-[^/]+\/credentials\.json$/)
const receipt = JSON.parse(await readFile(`${dirname(path)}/receipt.json`, 'utf8'))
assert.equal(receipt.directory, dirname(path))
const fixture = JSON.parse(await readFile(path, 'utf8'))
const name = process.argv[process.argv.indexOf('--user') + 1]
const user = fixture.users.find(u => u.username === name)
assert(user, 'Pass --user ios_alex or --user ios_blair')
assert(fixture.livekit_configured)
const base = fixture.origin, channel = fixture.dm_channel_id
const r = await fetch(`${base}/calls/${channel}/token`, { method: 'POST', headers: { Authorization: `Bearer ${user.session.token}` } })
assert(r.ok, `Media token status ${r.status}`)
const media = await r.json()
const browser = await chromium.launch({ executablePath: '/usr/bin/chromium', headless: true, args: ['--no-sandbox', '--use-fake-device-for-media-stream', '--use-fake-ui-for-media-stream', '--autoplay-policy=no-user-gesture-required', '--enable-usermedia-screen-capturing', '--auto-select-desktop-capture-source=Entire screen'] })
let stop
const stopped = new Promise(resolve => { stop = resolve })
process.once('SIGINT', () => stop('SIGINT'))
process.once('SIGTERM', () => stop('SIGTERM'))
const timer = setTimeout(() => stop('30-minute limit'), 30 * 60 * 1000)
try {
  const context = await browser.newContext({ permissions: ['microphone', 'camera'] })
  const page = await context.newPage()
  await page.route(base + '/fixture-peer', route => route.fulfill({ contentType: 'text/html', body: '<!doctype html><title>Den native test peer</title><body>Disposable native media peer</body>' }))
  await page.goto(base + '/fixture-peer')
  await page.addScriptTag({ path: new URL('../apps/web/node_modules/livekit-client/dist/livekit-client.umd.js', import.meta.url).pathname })
  await page.evaluate(async ({ media, screen }) => {
    const LK = window.LivekitClient
    window.room = new LK.Room()
    room.on(LK.RoomEvent.TrackSubscribed, track => document.body.append(track.attach()))
    await room.connect(media.url, media.token)
    for (const track of await LK.createLocalTracks({ audio: true, video: true })) await room.localParticipant.publishTrack(track)
    if (screen) await room.localParticipant.setScreenShareEnabled(true)
  }, { media, screen: process.argv.includes('--screen') })
  console.log(`READY: ${name}, disposable DM audio/camera${process.argv.includes('--screen') ? '/screen' : ''}; Ctrl-C to stop`)
  console.log(`STOP: ${await stopped}`)
} finally {
  clearTimeout(timer)
  await browser.close()
  console.log('Closed this peer only; fixture and other participants retained')
}
