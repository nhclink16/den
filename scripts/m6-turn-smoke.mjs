import { chromium } from 'playwright-core'
import { readFile } from 'node:fs/promises'
import assert from 'node:assert/strict'

const base = 'https://den.nicholascaron.com'
const credentials = JSON.parse(await readFile(process.env.DEN_SMOKE_CREDENTIALS, 'utf8'))
const browser = await chromium.launch({ executablePath: '/usr/bin/chromium', headless: true, args: [
  '--no-sandbox', '--use-fake-ui-for-media-stream', '--use-fake-device-for-media-stream',
  '--autoplay-policy=no-user-gesture-required',
] })
const peers = []
try {
  for (const [index, username] of ['m6_bob', 'm6_ari'].entries()) {
    const context = await browser.newContext({ permissions: ['microphone', 'camera'] })
    const page = await context.newPage()
    await page.goto(base)
    await page.addScriptTag({ path: 'apps/web/node_modules/livekit-client/dist/livekit-client.umd.js' })
    await page.evaluate(async ({ username, password, relay, forceTLS }) => {
      const response = await fetch('/auth/login', { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ username, password }) })
      if (!response.ok) throw new Error('Login failed')
      const session = await response.json()
      const headers = { Authorization: `Bearer ${session.token}` }
      const channels = await (await fetch('/channels', { headers })).json()
      const hangout = channels.find((c) => c.name === 'hangout')
      const call = await (await fetch(`/calls/${hangout.id}/token`, { method: 'POST', headers })).json()
      const Original = RTCPeerConnection
      function tlsOnly(config) {
        return { ...config, iceServers: (config?.iceServers || []).flatMap((server) => {
          const urls = [server.urls].flat().filter((url) => url.startsWith('turns:'))
          return urls.length ? [{ ...server, urls }] : []
        }) }
      }
      window.pcs = []
      window.RTCPeerConnection = class extends Original {
        constructor(config, ...args) {
          super(forceTLS ? tlsOnly(config) : config, ...args); window.pcs.push(this)
        }
        setConfiguration(config) { super.setConfiguration(forceTLS ? tlsOnly(config) : config) }
      }
      window.room = new LivekitClient.Room()
      room.on(LivekitClient.RoomEvent.TrackSubscribed, (track) => document.body.append(track.attach()))
      // Exercise LiveKit's actual connection option, not an app feature flag.
      await room.connect(call.url, call.token, relay ? { rtcConfig: { iceTransportPolicy: 'relay' } } : {})
      await room.localParticipant.setMicrophoneEnabled(true)
      await room.localParticipant.setCameraEnabled(true)
    }, { username, password: credentials.users[username], relay: index === 0, forceTLS: index === 0 && process.env.DEN_TURN_TLS === '1' })
    peers.push(page)
  }
  for (const [index, page] of peers.entries()) {
    await page.waitForFunction(async () => {
      const reports = await Promise.all(window.pcs.map((pc) => pc.getStats()))
      const rows = reports.flatMap((r) => [...r.values()])
      return rows.some((s) => s.type === 'inbound-rtp' && s.kind === 'audio' && s.bytesReceived > 1000) &&
        rows.some((s) => s.type === 'inbound-rtp' && s.kind === 'video' && s.framesDecoded > 0)
    }, undefined, { timeout: 30000 })
    const selected = await page.evaluate(async () => {
      const reports = await Promise.all(window.pcs.map((pc) => pc.getStats()))
      return reports.flatMap((report) => [...report.values()].filter((s) => s.type === 'transport' && s.selectedCandidatePairId).map((s) => {
        const pair = report.get(s.selectedCandidatePairId)
        const local = report.get(pair.localCandidateId), remote = report.get(pair.remoteCandidateId)
        return { localType: local.candidateType, localAddress: local.address, relayProtocol: local.relayProtocol, url: local.url, remoteAddress: remote.address, bytesReceived: pair.bytesReceived, bytesSent: pair.bytesSent }
      }))
    })
    assert(selected.length > 0)
    assert(selected.every((s) => s.remoteAddress === '135.148.120.197'))
    if (index === 0) assert(selected.every((s) => s.localType === 'relay'), 'relay-only client selected TURN for every transport')
    if (index === 0 && process.env.DEN_TURN_TLS === '1') assert(selected.every((s) => s.url?.startsWith('turns:') && s.relayProtocol === 'tls'), 'TURN/TLS selected')
    console.log(index === 0 ? 'Relay-only peer:' : 'Direct peer:', JSON.stringify(selected))
  }
  console.log('TURN smoke passed: relay-only ICE via LiveKit rtcConfig, two-way received audio and decoded video over public IP.')
} finally {
  for (const page of peers) await page.evaluate(() => window.room?.disconnect()).catch(() => {})
  await browser.close()
}
