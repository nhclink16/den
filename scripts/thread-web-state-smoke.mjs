// Draft ownership and read-response ordering, in a real browser against an
// isolated server. Local data only: it creates its own rooms and its own second
// user, and never touches the shared instance.
//
// DEN_SMOKE_URL selects the instance. PHASE=red points it at a build with the
// read-ordering version guard removed; that run is the control for the read
// guard ONLY, and it is expected to lose the count in `race`. It says nothing
// about draft behaviour, which is identical in both builds.
import { chromium } from 'playwright-core'
import { mkdir, mkdtemp, rm, writeFile, readFile } from 'node:fs/promises'
import { createHash, randomBytes } from 'node:crypto'
import assert from 'node:assert/strict'

const base = process.env.DEN_SMOKE_URL || 'http://localhost:17031'
const phase = process.env.PHASE === 'red' ? 'red' : 'green'
const guarded = phase === 'green'
const shots = new URL('../docs/shots/pr/thread-web-state/', import.meta.url).pathname
await mkdir(shots, { recursive: true })

const sha = async (path) => createHash('sha256').update(await readFile(path)).digest('hex')

let token
const api = async (method, path, body, as = token) => {
  const r = await fetch(base + path, {
    method,
    headers: { 'content-type': 'application/json', ...(as ? { authorization: `Bearer ${as}` } : {}) },
    body: body ? JSON.stringify(body) : undefined,
  })
  const payload = r.status === 204 ? '' : await r.text()
  assert(r.ok, `${method} ${path}: ${r.status} ${payload}`)
  return payload ? JSON.parse(payload) : null
}

const results = { phase, base, provenance: {}, steps: {} }
/// Ends the control run once it has made its one claim.
class ControlDone extends Error {}
const record = (name, value) => { results.steps[name] = value; console.log(`  ${name}:`, JSON.stringify(value)) }
let browser, scratch

try {
  // --- Provenance -----------------------------------------------------------
  // A port and a phase label do not say what build ran. These do.
  results.provenance = {
    at: new Date().toISOString(),
    serverBinary: process.env.DEN_SMOKE_SERVER || null,
    serverSha256: process.env.DEN_SMOKE_SERVER ? await sha(process.env.DEN_SMOKE_SERVER) : null,
    diffSha256: process.env.DEN_SMOKE_DIFF ? await sha(process.env.DEN_SMOKE_DIFF).catch(() => null) : null,
    redControlPatch: phase === 'red' ? process.env.DEN_SMOKE_RED_PATCH || null : null,
  }
  const served = await (await fetch(base + '/')).text()
  const bundle = served.match(/assets\/[A-Za-z0-9_-]+\.js/)?.[0]
  results.provenance.bundle = bundle
  results.provenance.bundleSha256 = createHash('sha256')
    .update(Buffer.from(await (await fetch(`${base}/${bundle}`)).arrayBuffer())).digest('hex')
  console.log('  provenance:', JSON.stringify(results.provenance))

  // --- 1. Fixture -----------------------------------------------------------
  // Two sessions for the same user: one for this script, one for the browser.
  // The lifetime scenario logs the browser out, which revokes only its own
  // session; sharing one would kill this script's API access halfway through.
  const login = await api('POST', '/auth/login', { username: 'nicholas', password: process.env.DEN_SMOKE_PASSWORD }, '')
  token = login.token
  const browserSession = await api('POST', '/auth/login',
    { username: 'nicholas', password: process.env.DEN_SMOKE_PASSWORD }, '')
  const tag = randomBytes(3).toString('hex')
  const room = await api('POST', '/channels', { name: `reading-${tag}`, kind: 'text', position: 0 })
  const other = await api('POST', '/channels', { name: `quiet-${tag}`, kind: 'text', position: 1 })
  const invite = await api('POST', '/invites', { uses: 1, expires_in_hours: 1 })
  const bob = await api('POST', '/auth/register',
    { username: `bob_${tag}`, password: randomBytes(24).toString('hex'), invite: invite.code }, '')
  // A second account, for the lifetime scenario: a send accepted under one
  // account must not touch the next one's state when it finally returns.
  const second = { username: `carol_${tag}`, password: randomBytes(24).toString('hex') }
  const carolInvite = await api('POST', '/invites', { uses: 1, expires_in_hours: 1 })
  await api('POST', '/auth/register', { ...second, invite: carolInvite.code }, '')
  const post = (content, opts = {}) => api('POST', `/channels/${room.id}/messages`, { content, ...opts }, bob.token)

  const seed = []
  for (let i = 0; i < 3; i++) seed.push(await post(`Seed ${i + 1} from bob`))
  record('fixture', { room: room.id, other: other.id, bob: bob.user.id, seeded: seed.length })

  browser = await chromium.launch({ executablePath: '/usr/bin/chromium', headless: true, args: ['--no-sandbox'] })
  const context = await browser.newContext({ viewport: { width: 1280, height: 860 }, colorScheme: 'dark' })
  await context.addCookies([{ name: 'den_session', value: browserSession.token, url: base, httpOnly: true, sameSite: 'Lax' }])
  await context.addInitScript((csrf) => localStorage.setItem('den.csrf', csrf), browserSession.csrf_token)
  const page = await context.newPage()

  // Unexpected failures are evidence too. The only thing this test causes on
  // purpose is one HTTP 503 on a send, so only that exact resource error, and
  // only while it is being caused, is expected. A JavaScript exception is never
  // expected, including during that window: an app that throws while handling a
  // rejected send is precisely the kind of failure worth catching here.
  const pageErrors = []
  let forcing = null
  page.on('pageerror', (e) => pageErrors.push({ kind: 'exception', during: forcing, expected: false, text: String(e) }))
  page.on('console', (m) => {
    if (m.type() !== 'error') return
    const text = m.text()
    const expected = forcing !== null && /Failed to load resource.*\b503\b/.test(text)
    pageErrors.push({ kind: 'console', during: forcing, expected, text })
  })

  // Mark-read interception. `arm` holds the NEXT read; everything else passes
  // straight through, so unrelated acknowledgements do not accumulate.
  const reads = []
  let arm = null
  const delivered = []
  page.on('response', async (r) => {
    if (!r.url().endsWith(`/channels/${room.id}/read`) || r.request().method() !== 'PUT') return
    delivered.push({ status: r.status(), body: await r.text().catch(() => '<unreadable>') })
  })
  await page.route(`**/channels/${room.id}/read`, async (route) => {
    if (route.request().method() !== 'PUT') return route.continue()
    const entry = { marker: JSON.parse(route.request().postData() || '{}').message_id, at: Date.now() }
    const hold = arm
    arm = null
    let release
    entry.held = new Promise((r) => { release = r })
    entry.release = release
    // A held read is fetched BEFORE it is announced when the test needs the
    // server to have really applied it: announcing first lets the next message
    // race the read, and the held body comes back already agreeing with it —
    // late, but not stale, which proves nothing.
    if (hold?.fetchFirst) { entry.response = await route.fetch(); entry.body = await entry.response.json() }
    reads.push(entry)
    if (!hold) release()
    await entry.held
    await route.fulfill({ response: entry.response ?? await route.fetch() })
  })
  const waitForReads = async (n, why) => {
    for (let i = 0; i < 120 && reads.length < n; i++) await page.waitForTimeout(50)
    assert.equal(reads.length, n, `${why}: expected ${n} mark-read requests, saw ${reads.length}`)
  }
  const badge = async (id) => (await page.locator(`a[href="/c/${id}"] .count`).count())
    ? page.locator(`a[href="/c/${id}"] .count`).textContent() : null
  const cursor = async () => (await api('GET', '/users/me/read-state')).find((s) => s.channel_id === room.id)

  arm = { fetchFirst: true }
  await page.goto(`${base}/c/${room.id}`, { waitUntil: 'domcontentloaded' })
  await page.locator('.msg').first().waitFor()

  // --- 2. The race: a socket update must beat the stale response it overtook -
  await waitForReads(1, 'opening the room')
  // The body now being held really is stale: the server has applied this read
  // through what was on screen, so it says zero, and the page is about to be
  // told otherwise by the socket.
  record('heldBody', reads[0].body)
  assert.equal(reads[0].marker, seed.at(-1).id, 'the read acknowledged something other than the displayed tail')
  assert.equal(reads[0].body.unread_count, 0, 'the held response was not a stale zero')
  const fresh = await post('bob says something new while your read is in flight')
  await page.waitForFunction((sel) => document.querySelector(sel)?.textContent === '1',
    `a[href="/c/${room.id}"] .count`, { timeout: 5000 }).catch(() => {})
  record('duringHeldRead', {
    badge: await badge(room.id), reads: reads.length,
    put: reads[0].marker, displayedAtOpen: seed.at(-1).id, newer: fresh.id,
  })
  assert.equal(await badge(room.id), '1', 'the socket update never reached the badge')

  await page.waitForTimeout(300)
  assert.equal(reads.length, 1, 'a second mark-read was dispatched while one was in flight')

  // Hold whatever the queue dispatches next as well. Letting it run would clear
  // the badge for an honest reason and hide what the stale body did.
  arm = { fetchFirst: false }
  reads[0].release()
  await page.waitForTimeout(600)
  const afterStale = await badge(room.id)
  record('staleResponseDelivered', delivered[0])
  assert.equal(delivered[0]?.status, 200, 'the held response never reached the page at all')
  record('afterStaleResponse', { badge: afterStale, expected: guarded ? '1' : 'anything but 1' })
  await page.screenshot({ path: `${shots}race-${phase}.png` })
  if (guarded) {
    assert.equal(afterStale, '1', 'a stale write overwrote a newer count')
  } else {
    // Without the version token two stale writers can win this: the held reply
    // itself, and the boot resync snapshot queued behind it. Which one lands
    // first is timing; that the newer count does not survive is the point.
    assert.notEqual(afterStale, '1', 'the control build was expected to lose the count')
  }

  await waitForReads(2, 'the read queued behind the held one')
  reads[1].release()
  await page.waitForTimeout(600)
  record('afterQueuedRead', { badge: await badge(room.id), reads: reads.length, marker: reads[1].marker })
  assert.equal(await badge(room.id), null, 'the queued mark-read never cleared the room')
  const settled = await cursor()
  assert.equal(settled.last_read_id, fresh.id, 'the server cursor did not reach the displayed marker')
  record('serverCursor', { last_read_id: settled.last_read_id, displayed: fresh.id, unread: settled.unread_count })

  if (!guarded) {
    // The control build differs by one line in applyIf, which also changes how
    // much read-state churn the rest of a session sees. It is evidence for the
    // ordering guard and nothing else: the scenarios below are run and asserted
    // only against the real build.
    results.note = 'Control for the read-ordering guard only. Not a before/after for drafts, quotes or uploads.'
    results.ok = true
    throw new ControlDone()
  }

  // --- 3. A queued read acknowledges what was displayed when it was QUEUED ---
  // Hold a read, queue another while B is the tail, leave the room, let C arrive,
  // then release. The queued read must acknowledge at most B: C was never shown.
  arm = { fetchFirst: false }
  await post('A, dispatched and held')
  await waitForReads(3, 'the held read for A')
  const b = await post('B, displayed while the next read is queued')
  await page.waitForTimeout(500)
  await page.locator(`a[href="/c/${other.id}"]`).click()
  await page.locator('textarea').waitFor()
  const c = await post('C, arrives after the room was left')
  await page.waitForTimeout(500)
  reads[2].release()
  await waitForReads(4, 'the read queued through B')
  await page.waitForTimeout(600)
  const afterMarker = await cursor()
  record('queuedReadMarker', {
    displayedWhenQueued: b.id, arrivedAfterLeaving: c.id,
    put: reads[3].marker, serverCursor: afterMarker.last_read_id, unread: afterMarker.unread_count,
  })
  assert.equal(reads[3].marker, b.id, 'the queued read acknowledged a message that was never displayed')
  assert.equal(afterMarker.last_read_id, b.id, 'the server cursor moved past what was displayed')
  assert.equal(afterMarker.unread_count, 1, 'C should still be unread')

  await page.locator(`a[href="/c/${room.id}"]`).click()
  await page.locator('.msg').first().waitFor()
  await page.waitForTimeout(500)

  // --- 4. No request loop ---------------------------------------------------
  const before = reads.length
  await page.waitForTimeout(2500)
  record('idleReadRequests', reads.length - before)
  assert.ok(reads.length - before <= 1, `mark-read looped while idle: ${reads.length - before} in 2.5s`)

  // --- 5. The draft outlives its composer -----------------------------------
  // Shell.svelte wraps ChannelView in {#key store.origin + currentChannel.id},
  // so navigating really does remount it; the smoke records the element identity
  // rather than taking that on trust.
  const ta = page.locator('textarea')
  const identity = () => page.locator('textarea').evaluate((el) => {
    const w = window
    w.__seen = w.__seen || []
    let i = w.__seen.indexOf(el)
    if (i < 0) { i = w.__seen.length; w.__seen.push(el) }
    return i
  })
  await ta.click()
  await ta.type('half written thought')
  const beforeNav = await identity()
  await page.screenshot({ path: `${shots}draft-before-nav-${phase}.png` })
  await page.locator(`a[href="/c/${other.id}"]`).click()
  await page.locator('textarea').waitFor()
  const otherBox = await page.locator('textarea').inputValue()
  await page.locator(`a[href="/c/${room.id}"]`).click()
  await page.locator('.msg').first().waitFor()
  const restored = await page.locator('textarea').inputValue()
  const afterNav = await identity()
  await page.screenshot({ path: `${shots}draft-after-nav-${phase}.png` })
  record('draftAcrossNavigation', { otherRoomBox: otherBox, restored, remounted: beforeNav !== afterNav })
  assert.equal(otherBox, '', 'the draft leaked into another room')
  assert.equal(restored, 'half written thought', 'the draft did not survive the composer unmounting')
  assert.notEqual(beforeNav, afterNav, 'the composer was never actually remounted')

  // --- 6. Pickers and the textarea binding ----------------------------------
  await ta.click()
  await ta.fill('')
  // A prefix unique to this run: the instance keeps earlier runs' users, and the
  // picker would otherwise offer whichever `bob_*` sorts first.
  await ta.type(`hello @bob_${tag.slice(0, 4)}`)
  await page.locator('[id^="mention-"]').first().waitFor({ timeout: 4000 })
  await page.keyboard.press('Enter')
  await page.waitForTimeout(150)
  const picked = await ta.inputValue()
  const caret = await ta.evaluate((el) => el.selectionStart)
  record('mentionPicker', { text: picked, caret })
  assert.equal(picked, `hello @bob_${tag} `, `mention was not inserted: ${picked}`)
  assert.equal(caret, picked.length, 'the caret did not follow the inserted mention')
  assert.equal(await ta.evaluate((el) => el.disabled), false, 'the textarea was disabled')

  await ta.fill('')
  await ta.type('/can')
  await page.locator('[id^="slash-"]').first().waitFor({ timeout: 4000 })
  const offered = await page.locator('[id^="slash-"]').allTextContents()
  await page.keyboard.press('Enter')
  await page.waitForTimeout(150)
  record('slashPicker', { offered: offered.length, text: await ta.inputValue() })
  assert.equal(await ta.inputValue(), '/canvas ', 'the slash picker did not complete the command')

  // The textarea's function binding (#32): a programmatic value plus an input
  // event is the path DictationButton's `update` callback ends on. This exercises
  // that binding and the draft owner, not Whisper; DictationButton update ->
  // setText is wired by inspection of Composer.svelte and is not executed here.
  await ta.fill('')
  await page.evaluate(() => {
    const el = document.querySelector('textarea')
    Object.getOwnPropertyDescriptor(HTMLTextAreaElement.prototype, 'value').set.call(el, 'dictated words here')
    el.dispatchEvent(new Event('input', { bubbles: true }))
    el.setSelectionRange(8, 8)
  })
  await page.waitForTimeout(150)
  const boundCaret = await ta.evaluate((el) => el.selectionStart)
  await page.locator(`a[href="/c/${other.id}"]`).click()
  await page.locator('textarea').waitFor()
  await page.locator(`a[href="/c/${room.id}"]`).click()
  await page.locator('.msg').first().waitFor()
  const boundAfterNav = await page.locator('textarea').inputValue()
  record('textareaBinding', { caret: boundCaret, afterNavigation: boundAfterNav })
  assert.equal(boundCaret, 8, 'the caret was not where the setter put it')
  assert.equal(boundAfterNav, 'dictated words here', 'the bound value did not reach the draft owner')

  // --- 7. A delayed accepted send, with the box still being edited ----------
  let releaseSend, sendAccepted
  const accepted = new Promise((r) => { sendAccepted = r })
  const sendHeld = new Promise((r) => { releaseSend = r })
  const sends = []
  await page.route(`**/channels/${room.id}/messages`, async (route) => {
    if (route.request().method() !== 'POST') return route.continue()
    const sent = JSON.parse(route.request().postData() || '{}')
    if (sends.length > 0) { sends.push({ sent }); return route.fulfill({ response: await route.fetch() }) }
    const response = await route.fetch()
    const body = await response.json()
    sends.push({ sent, body })
    sendAccepted(body)          // announced only once the server has persisted it
    await sendHeld
    await route.fulfill({ response })
  })
  await page.locator('textarea').fill('')
  await page.locator('textarea').type('sent while editing')
  await page.keyboard.press('Enter')
  const persisted = await accepted
  // The box stays editable while the send is open; it is only cleared on success,
  // so this appends to the text that was submitted.
  await page.locator('textarea').type(' and then more')
  await page.locator(`a[href="/c/${other.id}"]`).click()
  await page.locator('textarea').waitFor()
  await page.locator(`a[href="/c/${room.id}"]`).click()
  await page.locator('.msg').first().waitFor()
  releaseSend()
  await page.waitForTimeout(700)
  const copies = (await api('GET', `/channels/${room.id}/messages?limit=50`))
    .filter((m) => m.content === 'sent while editing')
  record('delayedSend', {
    accepted: persisted.content, kept: await page.locator('textarea').inputValue(), persistedCopies: copies.length,
  })
  assert.equal(persisted.content, 'sent while editing', 'the accepted body was not the submitted content')
  assert.equal(copies.length, 1, `the held send persisted ${copies.length} copies`)
  assert.equal(await page.locator('textarea').inputValue(), 'sent while editing and then more',
    'the late send cleared a draft that had been edited')
  await page.unroute(`**/channels/${room.id}/messages`)

  // --- 7b. A send accepted under one account, released under the next --------
  // The server has already taken this write; that stands. What must not follow
  // is the completion moving the SECOND account's cache or read cursor, which is
  // what an acknowledgement issued under the new epoch would do.
  let releaseAcross, acrossAccepted
  const acrossReady = new Promise((r) => { acrossAccepted = r })
  const acrossHeld = new Promise((r) => { releaseAcross = r })
  await page.route(`**/channels/${room.id}/messages`, async (route) => {
    if (route.request().method() !== 'POST') return route.continue()
    const response = await route.fetch()
    acrossAccepted(await response.json())
    await acrossHeld
    await route.fulfill({ response })
  })
  await page.locator('textarea').fill('')
  await page.locator('textarea').type('held across a logout')
  await page.keyboard.press('Enter')
  const acrossBody = await acrossReady

  // Log out and sign in as the second account, through the real UI.
  const signOut = async () => {
    await page.locator('a[href="/settings"]').click()
    await page.locator('a[href="/settings/account"]').click()
    await page.locator('button:has-text("Log out")').click()
    await page.locator('input[autocomplete="username"]').waitFor({ timeout: 15000 })
  }
  await signOut()
  await page.locator('input[autocomplete="username"]').fill(second.username)
  await page.locator('input[type=password]').fill(second.password)
  await page.locator('button[type=submit]').click()
  await page.locator(`a[href="/c/${room.id}"]`).waitFor({ timeout: 15000 })

  // Give the new account something to lose: it reads the room, leaves it, and
  // then a new message arrives there while it is elsewhere.
  await page.locator(`a[href="/c/${room.id}"]`).click()
  await page.locator('.msg').first().waitFor()
  await page.waitForTimeout(900)
  await page.locator(`a[href="/c/${other.id}"]`).click()
  await page.locator('textarea').waitFor()
  await page.locator('textarea').fill('')
  await page.locator('textarea').type('the second account is typing')
  const afterSwitch = await post('a message the second account has not read')
  await page.waitForFunction((sel) => document.querySelector(sel)?.textContent === '1',
    `a[href="/c/${room.id}"] .count`, { timeout: 8000 })
  const readsBefore = reads.length

  releaseAcross()
  await page.waitForTimeout(1200)
  const across = {
    acceptedAs: acrossBody.author_id, secondAccount: second.username, unreadAfterSwitch: afterSwitch.id,
    readPutsAfterSwitch: reads.length - readsBefore,
    badge: await badge(room.id),
    draft: await page.locator('textarea').inputValue(),
  }
  record('sendReleasedUnderNextAccount', across)
  assert.equal(across.readPutsAfterSwitch, 0, "the old send acknowledged a read under the new account")
  assert.equal(across.badge, '1', "the old send cleared the new account's unread count")
  assert.equal(across.draft, 'the second account is typing', "the old send cleared the new account's draft")
  await page.unroute(`**/channels/${room.id}/messages`)

  // Back to the original account for the remaining scenarios.
  await signOut()
  await page.locator('input[autocomplete="username"]').fill('nicholas')
  await page.locator('input[type=password]').fill(process.env.DEN_SMOKE_PASSWORD)
  await page.locator('button[type=submit]').click()
  await page.locator(`a[href="/c/${room.id}"]`).waitFor({ timeout: 15000 })
  await page.locator(`a[href="/c/${room.id}"]`).click()
  await page.locator('.msg').first().waitFor()
  await page.waitForTimeout(600)

  // --- 8. A failed send keeps text and files; the retry consumes exactly them -
  scratch = await mkdtemp('/tmp/den-web-state-')
  const png = `${scratch}/fixture.png`
  await writeFile(png, Buffer.from(
    'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNk+M9QDwADhgGAWjR9awAAAABJRU5ErkJggg==',
    'base64'))
  // A file staged in the other room, to show the retry consumes only its own.
  await page.locator(`a[href="/c/${other.id}"]`).click()
  await page.locator('textarea').waitFor()
  await page.locator('input[type=file]').setInputFiles(png)
  await page.waitForFunction(() => document.querySelectorAll('.fname').length === 1, null, { timeout: 10000 })
  const otherChips = await page.locator('.fname').count()
  await page.locator(`a[href="/c/${room.id}"]`).click()
  await page.locator('.msg').first().waitFor()

  await page.locator('textarea').fill('')
  await page.locator('textarea').type('this send will fail')
  await page.locator('input[type=file]').setInputFiles([png, png])
  await page.waitForFunction(() => document.querySelectorAll('.fname').length === 2, null, { timeout: 10000 })
  await page.waitForFunction(() => !document.querySelector('.sendbtn')?.disabled, null, { timeout: 10000 })

  const posts = []
  forcing = 'deliberate 503 on send'
  await page.route(`**/channels/${room.id}/messages`, async (route) => {
    if (route.request().method() !== 'POST') return route.continue()
    posts.push(JSON.parse(route.request().postData() || '{}'))
    return posts.length === 1
      ? route.fulfill({ status: 503, contentType: 'application/json', body: '{"error":"unavailable","message":"nope"}' })
      : route.fulfill({ response: await route.fetch() })
  })
  await page.locator('textarea').click()
  await page.keyboard.press('Enter')
  await page.waitForTimeout(800)
  const failed = {
    text: await page.locator('textarea').inputValue(),
    files: await page.locator('.fname').count(),
    error: await page.locator('.composer .err, .composer [role="alert"]').count(),
    uploadIds: posts[0]?.upload_ids ?? [],
  }
  record('failedSend', failed)
  assert.equal(failed.text, 'this send will fail', 'a failed send threw away the text')
  assert.equal(failed.files, 2, 'a failed send threw away the staged files')
  assert.ok(failed.error >= 1, 'the failure was not shown')
  assert.equal(failed.uploadIds.length, 2, 'the rejected send carried no upload ids')

  await page.keyboard.press('Enter')
  await page.waitForTimeout(1000)
  forcing = null
  const retry = {
    uploadIds: posts[1]?.upload_ids ?? [],
    remaining: await page.locator('.fname').count(),
    text: await page.locator('textarea').inputValue(),
  }
  await page.locator(`a[href="/c/${other.id}"]`).click()
  await page.locator('textarea').waitFor()
  retry.otherRoomChips = await page.locator('.fname').count()
  await page.locator(`a[href="/c/${room.id}"]`).click()
  await page.locator('.msg').first().waitFor()
  record('retryAfterFailure', retry)
  assert.deepEqual(retry.uploadIds, failed.uploadIds, 'the retry sent different uploads')
  assert.equal(retry.remaining, 0, 'the successful retry did not consume what it transmitted')
  assert.equal(retry.text, '', 'the successful retry did not clear its own draft')
  assert.equal(retry.otherRoomChips, otherChips, "the retry consumed another conversation's file")
  await page.unroute(`**/channels/${room.id}/messages`)

  // --- 9. A quote outside the loaded page survives to the wire --------------
  // Reply to a message, then push it out of the cached page the way a real
  // resync does, navigate away and back, and send. The quote belongs to the
  // draft, not to the message list: losing it silently turns a reply into a new
  // message.
  const quoted = seed[0]
  await page.locator(`#m-${quoted.id}`).hover()
  await page.locator(`#m-${quoted.id} button[title="Reply"]`).click({ force: true })
  await page.locator('.reply-bar').waitFor({ timeout: 4000 })
  await page.locator('textarea').fill('')
  await page.locator('textarea').type('answering the very first one')

  for (let i = 0; i < 55; i++) await post(`Filler ${i + 1}`)
  // A message in a channel the page has never seen makes the client resync,
  // which reloads the newest page and drops the quoted message from cache.
  const late = await api('POST', '/channels', { name: `late-${tag}`, kind: 'text', position: 2 })
  await api('POST', `/channels/${late.id}/messages`, { content: 'hello from a new room' }, bob.token)
  await page.waitForTimeout(3000)
  const cached = await page.evaluate((id) => !!document.getElementById(`m-${id}`), quoted.id)

  await page.locator(`a[href="/c/${other.id}"]`).click()
  await page.locator('textarea').waitFor()
  await page.locator(`a[href="/c/${room.id}"]`).click()
  await page.locator('textarea').waitFor()
  await page.waitForTimeout(800)
  const quoteSends = []
  await page.route(`**/channels/${room.id}/messages`, async (route) => {
    if (route.request().method() !== 'POST') return route.continue()
    quoteSends.push(JSON.parse(route.request().postData() || '{}'))
    return route.fulfill({ response: await route.fetch() })
  })
  await page.locator('textarea').click()
  await page.keyboard.press('Enter')
  await page.waitForTimeout(900)
  record('quoteOutsideLoadedPage', {
    quoted: quoted.id, stillCached: cached, sentReplyTo: quoteSends[0]?.reply_to ?? null,
    content: quoteSends[0]?.content,
  })
  assert.equal(cached, false, 'the quoted message was still on screen; the scenario did not arise')
  assert.equal(quoteSends[0]?.reply_to, quoted.id, 'the quote was silently dropped from the draft')

  // Explicit cancel still clears it.
  const newest = (await api('GET', `/channels/${room.id}/messages?limit=1`))[0]
  await page.locator('textarea').fill('')
  await page.locator('textarea').type('a second answer')
  await page.locator(`#m-${newest.id}`).hover()
  await page.locator(`#m-${newest.id} button[title="Reply"]`).click({ force: true })
  await page.locator('.reply-bar').waitFor({ timeout: 4000 })
  await page.locator('.reply-bar button.x').click()
  await page.waitForTimeout(300)
  await page.locator('textarea').click()
  await page.keyboard.press('Enter')
  await page.waitForTimeout(900)
  record('explicitCancel', {
    banner: await page.locator('.reply-bar').count(), sent: quoteSends[1] ?? null,
  })
  // The send must have happened: comparing a missing POST's reply_to to null
  // would pass whether the quote was cancelled or the message never left.
  assert.ok(quoteSends[1], 'the second answer was never sent')
  assert.equal(quoteSends[1].content, 'a second answer', 'a different message was sent')
  assert.equal(quoteSends[1].reply_to ?? null, null, 'an explicitly cancelled quote was still sent')
  await page.unroute(`**/channels/${room.id}/messages`)

  // --- 10. The flat room is unchanged ---------------------------------------
  const reply = await post('a reply to the first seed', { reply_to: seed[0].id })
  await page.waitForTimeout(1000)
  record('flatListingShowsReplies', {
    reply: reply.id, thread_id: reply.thread_id ?? null,
    listed: await page.locator(`#m-${reply.id}`).count(),
  })
  assert.equal(await page.locator(`#m-${reply.id}`).count(), 1, 'a reply stopped appearing in the flat room')
  assert.ok(reply.thread_id, 'the reply carried no thread id, so this is not the threaded row it claims')

  await page.screenshot({ path: `${shots}room-${phase}.png` })
  results.pageErrors = pageErrors
  assert.deepEqual(pageErrors.filter((e) => !e.expected), [], 'the page reported unexpected errors')
  results.ok = true
} catch (e) {
  if (!(e instanceof ControlDone)) throw e
} finally {
  await browser?.close()
  if (scratch) await rm(scratch, { recursive: true, force: true })
  await writeFile(`${shots}results-${phase}.json`, JSON.stringify(results, null, 2) + '\n')
  console.log(`\n${phase}: ${results.ok ? 'PASS' : 'FAIL'} — evidence in docs/shots/pr/thread-web-state/`)
}
