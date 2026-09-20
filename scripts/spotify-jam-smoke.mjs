// Run through scripts/run-spotify-jam-smoke.sh. It creates an isolated server,
// two disposable people and no real Spotify traffic or credentials.
import { chromium } from 'playwright-core'
import { mkdir, readFile, writeFile } from 'node:fs/promises'
import assert from 'node:assert/strict'

const base = process.env.DEN_SMOKE_URL || 'http://127.0.0.1:5173'
const apiBase = process.env.DEN_SMOKE_API || 'http://127.0.0.1:7000'
const browserPath = process.env.DEN_CHROMIUM || '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome'
const shots = new URL('../docs/shots/pr/feat/spotify-jam/', import.meta.url).pathname
await mkdir(shots, { recursive: true })

let adminToken = ''
const api = async (method, path, body, token = adminToken) => {
  const response = await fetch(apiBase + path, {
    method,
    headers: { 'content-type': 'application/json', ...(token ? { authorization: `Bearer ${token}` } : {}) },
    body: body === undefined ? undefined : JSON.stringify(body),
  })
  const text = response.status === 204 ? '' : await response.text()
  assert(response.ok, `${method} ${path}: ${response.status} ${text}`)
  return text ? JSON.parse(text) : null
}

const bootstrap = (await readFile(process.env.DEN_SMOKE_BOOTSTRAP, 'utf8')).trim()
const admin = await api('POST', '/auth/init', {
  username: 'qa_admin', password: 'local-smoke-password', bootstrap_token: bootstrap,
}, '')
adminToken = admin.token
const invite = await api('POST', '/invites', { uses: 2, expires_in_hours: 1 })
const host = await api('POST', '/auth/register', {
  username: 'jam_host', password: 'local-smoke-password', invite: invite.code,
}, '')
const guest = await api('POST', '/auth/register', {
  username: 'jam_guest', password: 'local-smoke-password', invite: invite.code,
}, '')
const channels = await api('GET', '/channels')
const textRoom = channels.find(channel => channel.kind === 'text')
const voiceRoom = channels.find(channel => channel.kind === 'voice')
assert(textRoom && voiceRoom)

const browser = await chromium.launch({ executablePath: browserPath, headless: true })
const errors = []
const results = { commit: process.env.DEN_SMOKE_COMMIT || null, checks: {} }
const open = async (session, viewport = { width: 1440, height: 900 }) => {
  const context = await browser.newContext({ viewport, colorScheme: 'dark' })
  await context.route('https://spotify.link/**', route => route.abort())
  await context.addCookies([{
    name: 'den_session', value: session.token, url: base, httpOnly: true, sameSite: 'Lax',
  }])
  await context.addInitScript(csrf => localStorage.setItem('den.csrf', csrf), session.csrf_token)
  const page = await context.newPage()
  page.on('pageerror', error => errors.push(String(error)))
  page.on('console', message => { if (message.type() === 'error' && !message.text().includes('ERR_FAILED')) errors.push(message.text()) })
  page.setDefaultTimeout(15_000)
  return { context, page }
}

let hostBrowser, guestBrowser
try {
  hostBrowser = await open(host)
  guestBrowser = await open(guest)
  const hostPage = hostBrowser.page
  const guestPage = guestBrowser.page

  await hostPage.goto(`${base}/c/${textRoom.id}`, { waitUntil: 'domcontentloaded' })
  const composer = hostPage.getByLabel(`Say something in #${textRoom.name}`)
  await composer.fill('https://spotify.link/first-smoke')
  await hostPage.getByText(`Pin this Spotify Jam to the top of #${textRoom.name}?`).waitFor()
  await hostPage.getByRole('button', { name: 'Pin the Jam' }).click()
  const hostCard = hostPage.getByRole('region', { name: 'Spotify Jam' })
  await hostCard.waitFor()
  assert.match(await hostCard.innerText(), /Listening together/)
  assert.match(await hostCard.innerText(), /Started by jam_host/)
  assert.equal(await hostCard.getByRole('link', { name: /Open Spotify Jam in a new window/ }).count(), 1)
  await hostPage.screenshot({ path: `${shots}/desktop.png` })

  await guestPage.goto(`${base}/c/${textRoom.id}`, { waitUntil: 'domcontentloaded' })
  const guestCard = guestPage.getByRole('region', { name: 'Spotify Jam' })
  await guestCard.waitFor()
  const join = guestCard.getByRole('link', { name: /Join Spotify Jam in a new window/ })
  assert.equal(await join.count(), 1)
  await join.click({ noWaitAfter: true })
  await guestCard.getByRole('link', { name: /Open Spotify Jam in a new window/ }).waitFor()
  await hostCard.getByText(/2 people opened this Jam from Den/).waitFor()
  results.checks.sharedJoin = true

  // Inline confirmation stays keyboard-operable and restores focus to Keep.
  await hostCard.getByRole('button', { name: 'End' }).click()
  const keep = hostCard.getByRole('button', { name: 'Keep' })
  await keep.waitFor()
  assert.equal(await keep.evaluate(element => element === document.activeElement), true)
  await keep.press('Enter')

  // An event that happens while this client has no socket must appear after its
  // resync. Before the all-room refetch fix, the absent Jam stayed absent.
  await api('DELETE', `/rooms/${textRoom.id}/jam`, undefined, host.token)
  await hostCard.waitFor({ state: 'detached' })
  await hostPage.evaluate(async () => {
    const { store } = await import('/src/lib/store.svelte.ts')
    // Hold automatic reconnect until the server mutation is complete, so this
    // is a deterministic event gap rather than a race with the 800 ms backoff.
    const socket = store.ws
    socket.onclose = () => { store.connected = false; store.ws = null }
    socket.close()
  })
  await hostPage.getByLabel(/Offline, reconnecting/).waitFor()
  await api('POST', `/rooms/${textRoom.id}/jam`, { url: 'https://spotify.link/reconnect-smoke' }, guest.token)
  await hostPage.evaluate(async () => {
    const { store } = await import('/src/lib/store.svelte.ts')
    await store.connect()
  })
  await hostPage.getByRole('region', { name: 'Spotify Jam' }).waitFor()
  assert.match(await hostPage.getByRole('region', { name: 'Spotify Jam' }).innerText(), /Started by jam_guest/)
  results.checks.reconnectGap = true

  await hostPage.setViewportSize({ width: 320, height: 700 })
  await hostPage.waitForTimeout(150)
  const bounds = await hostPage.getByRole('region', { name: 'Spotify Jam' }).boundingBox()
  assert(bounds && bounds.x >= 0 && bounds.x + bounds.width <= 320)
  assert.equal(await hostPage.evaluate(() => document.documentElement.scrollWidth <= innerWidth), true)
  await hostPage.screenshot({ path: `${shots}/mobile-320.png` })
  results.checks.mobileReflow = bounds

  // Voice rooms keep a dedicated input. Invalid text is explained on submit,
  // and the enabled button gives keyboard users a validation path.
  await hostPage.setViewportSize({ width: 390, height: 844 })
  await hostPage.goto(`${base}/c/${voiceRoom.id}`, { waitUntil: 'domcontentloaded' })
  const jamInput = hostPage.getByLabel('Spotify Jam link')
  await jamInput.fill('https://open.spotify.com/track/not-a-jam')
  const pin = hostPage.getByRole('button', { name: 'Pin a Jam' })
  assert.equal(await pin.isEnabled(), true)
  await pin.click()
  await hostPage.getByText(/Paste a Spotify Jam link/).waitFor()
  assert.equal(await jamInput.evaluate(element => element === document.activeElement), true)
  results.checks.invalidLinkRecovery = true

  await hostPage.goto(`${base}/settings/spotify`, { waitUntil: 'domcontentloaded' })
  await hostPage.getByText('Not set up on this Den.').waitFor()
  await hostPage.screenshot({ path: `${shots}/settings-mobile.png` })

  // The client-only callback route must survive the thread-era router split and
  // scrub Spotify's query parameters before it renders the denial.
  await hostPage.goto(`${base}/spotify/callback?error=access_denied&state=private-state`, { waitUntil: 'domcontentloaded' })
  await hostPage.getByRole('heading', { name: 'Spotify did not connect' }).waitFor()
  assert.equal(new URL(hostPage.url()).search, '')
  assert.match(await hostPage.getByRole('alert').innerText(), /cancelled/)
  results.checks.callbackRoute = true

  await hostPage.goto(`${base}/c/${textRoom.id}`, { waitUntil: 'domcontentloaded' })
  await hostPage.getByRole('region', { name: 'Spotify Jam' }).waitFor()
  const cleared = await hostPage.evaluate(async () => {
    const { store } = await import('/src/lib/store.svelte.ts')
    await store.logout()
    return { jams: store.jams.size, spotify: store.spotify.connection }
  })
  assert.deepEqual(cleared, { jams: 0, spotify: 'unavailable' })
  results.checks.logoutIsolation = true

  assert.deepEqual(errors, [])
  await writeFile(`${shots}/verification.json`, `${JSON.stringify(results, null, 2)}\n`)
  console.log(`Spotify Jam smoke passed: ${JSON.stringify(results.checks)}`)
} finally {
  await hostBrowser?.context.close()
  await guestBrowser?.context.close()
  await browser.close()
}
