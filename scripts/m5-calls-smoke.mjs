// Real two-peer media and invitation lifecycle against an exact disposable fixture.
import { chromium } from 'playwright-core'
import { readFile } from 'node:fs/promises'
import { dirname, resolve } from 'node:path'
import assert from 'node:assert/strict'
const credentialsPath = resolve(process.env.DEN_IOS_FIXTURE_CREDENTIALS || '')
assert.match(credentialsPath, /^\/mnt\/storage\/den-ios-fixture-[^/]+\/credentials\.json$/)
const receipt = JSON.parse(await readFile(`${dirname(credentialsPath)}/receipt.json`, 'utf8'))
assert.equal(receipt.directory, dirname(credentialsPath))
const fixture = JSON.parse(await readFile(credentialsPath, 'utf8'))
assert(fixture.livekit_configured)
const base = fixture.origin, channel = fixture.dm_channel_id
const [caller, recipient] = fixture.users.map(u => u.session)
async function api(path, user, body) {
  const r = await fetch(base + path, { method: body === undefined ? 'GET' : 'POST', headers: { Authorization: `Bearer ${user.token}`, 'Content-Type': 'application/json' }, body: body === undefined ? undefined : JSON.stringify(body) })
  assert(r.ok, `${path}: ${r.status}`)
  return r.status === 204 ? null : r.json()
}
async function until(check, description) {
  const deadline = Date.now() + 20000
  while (Date.now() < deadline) { if (await check()) return; await new Promise(r => setTimeout(r, 100)) }
  throw new Error(`Timed out: ${description}`)
}
const browser = await chromium.launch({ executablePath: '/usr/bin/chromium', headless: true, args: ['--no-sandbox', '--use-fake-device-for-media-stream', '--use-fake-ui-for-media-stream', '--autoplay-policy=no-user-gesture-required'] })
const pages = []
let invitation
async function connect(user, publish = true) {
  const media = await api(`/calls/${channel}/token`, user, {})
  const context = await browser.newContext({ permissions: ['microphone', 'camera'] })
  const page = await context.newPage(); pages.push(page)
  await page.route(base + '/fixture-smoke', route => route.fulfill({ contentType: 'text/html', body: '<!doctype html><title>Isolated media smoke</title><body></body>' }))
  await page.goto(base + '/fixture-smoke')
  await page.addScriptTag({ path: new URL('../apps/web/node_modules/livekit-client/dist/livekit-client.umd.js', import.meta.url).pathname })
  await page.evaluate(async ({ media, publish }) => {
    window.pcs = []
    const Original = window.RTCPeerConnection
    window.RTCPeerConnection = class extends Original { constructor(...args) { super(...args); window.pcs.push(this) } }
    const LK = window.LivekitClient
    window.room = new LK.Room()
    room.on(LK.RoomEvent.TrackSubscribed, track => document.body.append(track.attach()))
    await room.connect(media.url, media.token)
    if (publish) {
      const tracks = await LK.createLocalTracks({ audio: true, video: true })
      for (const track of tracks) await room.localParticipant.publishTrack(track)
    }
  }, { media, publish })
  return page
}
async function received(page) {
  return page.evaluate(async () => {
    const stats = (await Promise.all(pcs.map(pc => pc.getStats()))).flatMap(report => [...report.values()])
    return { audio: stats.filter(s => s.type === 'inbound-rtp' && s.kind === 'audio').reduce((n, s) => n + s.bytesReceived, 0), video: stats.filter(s => s.type === 'inbound-rtp' && s.kind === 'video').reduce((n, s) => n + s.framesDecoded, 0), addresses: stats.filter(s => s.type === 'remote-candidate').map(s => s.address) }
  })
}
try {
  const a = await connect(caller)
  invitation = await api(`/calls/${channel}/invite`, caller, {})
  const answer = { invitation_id: invitation.id, answer_id: crypto.randomUUID() }
  assert.equal((await api(`/calls/${channel}/invite/accept`, recipient, answer)).state, 'active')
  const b = await connect(recipient)
  await until(async () => { const states = await Promise.all([received(a), received(b)]); return states.every(s => s.audio > 0 && s.video > 0) }, 'bidirectional decoded audio and video')
  const initial = await Promise.all([received(a), received(b)])
  assert(initial.every(s => s.addresses.includes(fixture.livekit_media_ip)))
  await b.evaluate(() => room.disconnect())
  assert.equal((await api(`/calls/invitations/${invitation.id}`, recipient)).state, 'active')
  assert.equal((await api(`/calls/${channel}/invite/accept`, recipient, answer)).state, 'active')
  const reconnect = await connect(recipient)
  await until(async () => { const s = await received(reconnect); return s.audio > 0 && s.video > 0 }, 'media after recipient reconnect')
  const sibling = await connect(caller, false)
  await a.evaluate(() => room.disconnect())
  assert.equal((await api(`/calls/invitations/${invitation.id}`, caller)).state, 'active')
  await api(`/calls/${channel}/invite/end`, caller, { invitation_id: invitation.id })
  assert.equal((await api(`/calls/invitations/${invitation.id}`, recipient)).state, 'ended')
  assert.equal(await sibling.evaluate(() => room.state), 'connected')
  assert.equal(await reconnect.evaluate(() => room.state), 'connected')
  console.log('PASS: real two-peer audio/video, Tailnet media, accept/retry, reconnect, sibling preservation, signaling-only end')
} finally {
  for (const page of pages) await page.evaluate(() => window.room?.disconnect()).catch(() => {})
  await browser.close()
  if (invitation) await api(`/calls/${channel}/invite/end`, caller, { invitation_id: invitation.id })
  console.log('Cleanup: closed all owned media connections; fixture retained for native tests')
}
