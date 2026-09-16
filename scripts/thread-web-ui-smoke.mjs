// Thread activation acceptance: two people and an agent, against an isolated
// server. Local data only; it creates its own rooms, users and bot token and
// never touches the shared instance.
//
// BASELINE=1 runs against the unchanged tree: it captures the room at each
// viewport so the after-images have something real to compare against. It makes
// no claim that any thread feature works there, because none exists yet.
import { chromium } from 'playwright-core'
import { mkdir, mkdtemp, rm, writeFile, readFile } from 'node:fs/promises'
import { createHash, randomBytes } from 'node:crypto'
import assert from 'node:assert/strict'

const base = process.env.DEN_SMOKE_URL || 'http://localhost:17040'
const baseline = process.env.BASELINE === '1'
const phase = baseline ? 'baseline' : 'after'
const shots = new URL(`../docs/shots/pr/thread-web-ui/${phase}/`, import.meta.url).pathname
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
const record = (name, value) => { results.steps[name] = value; console.log(`  ${name}:`, JSON.stringify(value)) }
let browser, scratch

// Landscape, portrait and a narrow window. The pair-vs-single layout is chosen
// from available content space, so these are the three shapes that matter.
const VIEWPORTS = {
  landscape: { width: 1440, height: 900 },
  portrait: { width: 900, height: 1440 },
  narrow: { width: 720, height: 900 },
}

// Labelled so BASELINE can leave early: `finally` still writes the results.
run: try {
  results.provenance = {
    at: new Date().toISOString(),
    commit: process.env.DEN_SMOKE_COMMIT || null,
    diffSha256: process.env.DEN_SMOKE_DIFF ? await sha(process.env.DEN_SMOKE_DIFF).catch(() => null) : null,
    serverSha256: process.env.DEN_SMOKE_SERVER ? await sha(process.env.DEN_SMOKE_SERVER).catch(() => null) : null,
  }
  const served = await (await fetch(base + '/')).text()
  const bundle = served.match(/assets\/[A-Za-z0-9_-]+\.js/)?.[0]
  results.provenance.bundle = bundle
  results.provenance.bundleSha256 = createHash('sha256')
    .update(Buffer.from(await (await fetch(`${base}/${bundle}`)).arrayBuffer())).digest('hex')
  console.log('  provenance:', JSON.stringify(results.provenance))

  // --- Fixture: two people and an agent ------------------------------------
  const login = await api('POST', '/auth/login', { username: 'nicholas', password: process.env.DEN_SMOKE_PASSWORD }, '')
  token = login.token
  const browserSession = await api('POST', '/auth/login',
    { username: 'nicholas', password: process.env.DEN_SMOKE_PASSWORD }, '')
  const tag = randomBytes(3).toString('hex')
  const room = await api('POST', '/channels', { name: `workshop-${tag}`, kind: 'text', position: 0 })
  const other = await api('POST', '/channels', { name: `quiet-${tag}`, kind: 'text', position: 1 })
  const invite = await api('POST', '/invites', { uses: 1, expires_in_hours: 1 })
  const mira = await api('POST', '/auth/register',
    { username: `mira_${tag}`, password: randomBytes(24).toString('hex'), invite: invite.code }, '')
  const say = (content, opts = {}) => api('POST', `/channels/${room.id}/messages`, { content, ...opts }, mira.token)

  const roots = []
  roots.push(await say('The deploy script fails on the second run.'))
  roots.push(await say('Anyone want lunch?'))
  record('fixture', { room: room.id, other: other.id, mira: mira.user.id, roots: roots.map((m) => m.id) })

  browser = await chromium.launch({ executablePath: '/usr/bin/chromium', headless: true, args: ['--no-sandbox'] })
  const context = await browser.newContext({ viewport: VIEWPORTS.landscape, colorScheme: 'dark' })
  await context.addCookies([{ name: 'den_session', value: browserSession.token, url: base, httpOnly: true, sameSite: 'Lax' }])
  await context.addInitScript((csrf) => localStorage.setItem('den.csrf', csrf), browserSession.csrf_token)
  const page = await context.newPage()

  const pageErrors = []
  let forcing = null
  page.on('pageerror', (e) => pageErrors.push({ kind: 'exception', during: forcing, expected: false, text: String(e) }))
  page.on('console', (m) => {
    if (m.type() !== 'error') return
    const text = m.text()
    const expected = forcing !== null && /Failed to load resource.*\b503\b/.test(text)
    pageErrors.push({ kind: 'console', during: forcing, expected, text })
  })

  await page.goto(`${base}/c/${room.id}`, { waitUntil: 'domcontentloaded' })
  await page.locator('.msg').first().waitFor()
  await page.waitForTimeout(600)

  // --- Every viewport, in both phases --------------------------------------
  const shapes = {}
  for (const [name, size] of Object.entries(VIEWPORTS)) {
    await page.setViewportSize(size)
    await page.waitForTimeout(400)
    shapes[name] = {
      strip: await page.locator('[data-thread-strip]').count(),
      panel: await page.locator('[data-thread-panel]').count(),
      messages: await page.locator('.msg').count(),
    }
    await page.screenshot({ path: `${shots}room-${name}.png` })
  }
  record('viewports', shapes)

  if (baseline) {
    // The control, and nothing beyond it. This runs against the UNCHANGED #42
    // client, where no thread UI exists at all, so everything after this point
    // asserts behaviour that client cannot have. Stop here and say so, rather
    // than fail on the first conversation and call the control broken.
    for (const [name, s] of Object.entries(shapes)) {
      assert.equal(s.strip, 0, `${name}: the unchanged tree should have no thread strip`)
      assert.equal(s.panel, 0, `${name}: the unchanged tree should have no thread panel`)
    }
    results.note = 'Unchanged #42. No thread UI exists here; these are comparison images only.'
    results.ok = true
    break run
  }
  // Back to landscape for the scenarios; the narrow shape is revisited at the end.
  await page.setViewportSize(VIEWPORTS.landscape)
  await page.waitForTimeout(300)

  // ===================== 1. A conversation, and an agent in it ==============
  // A human asks something, someone replies, and an agent's job reports into
  // that same conversation instead of into the room.
  const agent = await api('POST', '/bots', { username: `helper${tag}`, display_name: 'Helper' })
  const bot = agent.credential.secret
  const asAgent = (content, extra = {}) =>
    api('POST', `/channels/${room.id}/messages`, { content, ...extra }, bot)

  await page.locator(`#m-${roots[0].id}`).hover()
  await page.locator(`#m-${roots[0].id} button[title="Reply"]`).click({ force: true })
  await page.locator('[data-thread-panel]').waitFor({ timeout: 5000 })
  const preCreationUrl = new URL(page.url())
  await page.locator('[data-thread-panel] textarea').fill('')
  await page.locator('[data-thread-panel] textarea').type('I can reproduce it. Looking now.')
  await page.keyboard.press('Enter')
  await page.waitForFunction(() => location.pathname.includes('/t/'), null, { timeout: 8000 })
  const threadA = new URL(page.url()).pathname.split('/t/')[1]

  // The agent's job replies to the human request, so the job joins that thread.
  const taskX = `job-${tag}-x`
  const first = await asAgent('Starting on the deploy failure.', { task_id: taskX, reply_to: roots[0].id })
  for (const step of ['Reproduced on run two.', 'Found it: the lockfile is not released.'])
    await asAgent(step, { task_id: taskX })
  // A second, unrelated job from the same agent must not join it.
  const taskY = `job-${tag}-y`
  const otherJob = await asAgent('Unrelated: rotating the backup keys.', { task_id: taskY })
  // A reply to a reply stays in the same conversation; it never nests.
  const nested = await api('POST', `/channels/${room.id}/messages`,
    { content: 'Nice find.', reply_to: first.id }, mira.token)
  await page.waitForTimeout(1200)

  const roomRoots = await api('GET', `/channels/${room.id}/messages?limit=50&roots_only=true`)
  const repliesA = await api('GET', `/threads/${threadA}/messages?limit=50`)
  const threadsHere = await api('GET', `/channels/${room.id}/threads?limit=50`)
  const threadY = threadsHere.find((v) => v.thread.root_message_id === otherJob.id)?.thread.id
  record('agentInConversation', {
    preCreationUrl: `${preCreationUrl.pathname}${preCreationUrl.search}`,
    threadA, threadY,
    roomRootIds: roomRoots.map((m) => m.id),
    repliesInA: repliesA.length,
    nestedStayedFlat: nested.thread_id === threadA,
    domRootsShown: await page.locator('.room .msg').count(),
  })
  assert.equal(preCreationUrl.searchParams.get('reply'), roots[0].id,
    'the panel before creation must be addressed by its root')
  assert.ok(threadA, 'the first reply did not create a conversation')
  assert.ok(threadY && threadY !== threadA, 'two jobs from one agent shared a conversation')
  assert.equal(nested.thread_id, threadA, 'a reply to a reply started a nested conversation')
  // Three agent progress posts and two human replies live in the conversation;
  // the room still holds only its roots.
  assert.equal(repliesA.length, 5, `expected 5 replies in the conversation, saw ${repliesA.length}`)
  assert.ok(!roomRoots.some((m) => m.id === first.id), 'agent progress leaked into the room feed')
  assert.equal(roomRoots.length, 3, 'the room should hold two seeds plus the second job root')

  // ===================== 2. Drafts, files and a held first send =============
  const threadB = threadY
  const draftsSeen = {}
  // Navigation must stay INSIDE the app: drafts live in memory, so a full page
  // load would clear them for entirely legitimate reasons and prove nothing.
  const openConversation = async (id) => {
    const chip = page.locator(`[data-thread-strip] .chip[data-thread="${id}"]`)
    if (await chip.count()) await chip.first().click()
    else await page.locator(`.room a.thread-link[href$="/t/${id}"]`).first().click()
    await page.locator('[data-thread-panel]').waitFor()
    await page.waitForTimeout(500)
  }
  const backToRoom = async () => {
    await page.locator('[data-thread-panel] button[aria-label="Close conversation"], [data-thread-panel] button[aria-label="Back to room"]').first().click()
    await page.waitForTimeout(400)
  }
  const type = async (where, text) => {
    await page.locator(`${where} textarea`).fill('')
    await page.locator(`${where} textarea`).type(text)
  }
  // Room draft, then conversation A, then conversation B.
  await page.goto(`${base}/c/${room.id}`, { waitUntil: 'domcontentloaded' })
  await page.locator('.room .msg').first().waitFor()
  await type('.room', 'a room draft nobody should touch')
  await openConversation(threadA)
  await type('[data-thread-panel]', 'draft in A')
  await openConversation(threadB)
  await type('[data-thread-panel]', 'draft in B')
  draftsSeen.b = await page.locator('[data-thread-panel] textarea').inputValue()
  await openConversation(threadA)
  draftsSeen.a = await page.locator('[data-thread-panel] textarea').inputValue()
  draftsSeen.room = await page.locator('.room textarea').inputValue()
  record('draftsAcrossConversations', draftsSeen)
  assert.equal(draftsSeen.a, 'draft in A', 'switching conversations lost a draft')
  assert.equal(draftsSeen.b, 'draft in B')
  assert.equal(draftsSeen.room, 'a room draft nobody should touch', 'a conversation clobbered the room draft')

  // A->B->A while a send is open, with the response held only AFTER the server
  // accepted it, and a failed send that must never reach the server.
  const posts = []
  let releaseSend, accepted
  const acceptedAt = new Promise((r) => { accepted = r })
  const held = new Promise((r) => { releaseSend = r })
  await page.route(`**/channels/${room.id}/messages`, async (route) => {
    if (route.request().method() !== 'POST') return route.continue()
    const sent = JSON.parse(route.request().postData() || '{}')
    posts.push(sent)
    if (posts.length > 1) return route.fulfill({ response: await route.fetch() })
    const response = await route.fetch()
    accepted(await response.json())
    await held
    await route.fulfill({ response })
  })
  await type('[data-thread-panel]', 'holding this one')
  await page.keyboard.press('Enter')
  const persisted = await acceptedAt
  await type('[data-thread-panel]', 'edited once')
  await type('[data-thread-panel]', 'holding this one')   // A -> B -> A: still two edits
  releaseSend()
  await page.waitForTimeout(800)
  record('heldSendWhileEditing', {
    sentBody: posts[0], acceptedId: persisted.id, kept: await page.locator('[data-thread-panel] textarea').inputValue(),
  })
  assert.equal(posts[0].thread_id, threadA, 'the reply was not placed in its conversation')
  assert.equal(await page.locator('[data-thread-panel] textarea').inputValue(), 'holding this one',
    'a late send cleared a draft that had been edited twice')
  await page.unroute(`**/channels/${room.id}/messages`)

  // A failed send must not reach the server at all.
  const before = (await api('GET', `/threads/${threadA}/messages?limit=50`)).length
  forcing = 'deliberate 503 on a reply'
  await page.route(`**/channels/${room.id}/messages`, (route) =>
    route.request().method() === 'POST'
      ? route.fulfill({ status: 503, contentType: 'application/json', body: '{"error":"unavailable","message":"nope"}' })
      : route.continue())
  await type('[data-thread-panel]', 'this one fails')
  await page.keyboard.press('Enter')
  await page.waitForTimeout(700)
  const afterFail = (await api('GET', `/threads/${threadA}/messages?limit=50`)).length
  record('failedReply', {
    serverRepliesBefore: before, serverRepliesAfter: afterFail,
    kept: await page.locator('[data-thread-panel] textarea').inputValue(),
    error: await page.locator('[data-thread-panel] .composer .error').count(),
  })
  assert.equal(afterFail, before, 'a failed send reached the server anyway')
  assert.equal(await page.locator('[data-thread-panel] textarea').inputValue(), 'this one fails',
    'a failed send threw away the draft')
  await page.unroute(`**/channels/${room.id}/messages`)
  forcing = null

  // ===================== 3. Unread that stays separate =====================
  // Leave the room entirely first: a conversation on screen is being read, which
  // would quietly undo the fixture.
  await page.goto(`${base}/c/${other.id}`, { waitUntil: 'domcontentloaded' })
  await page.locator('textarea').waitFor()
  // Relevance is following, DM membership or a channel subscription. This account
  // replied in A so it already follows A; B is the agent's own job, so following
  // it is the explicit act that makes its replies count.
  await api('PUT', `/threads/${threadB}/follow`, { following: true })
  // Read everything first, then make the room and both conversations unread.
  await api('PUT', `/channels/${room.id}/read`, { message_id: roomRoots.at(-1).id })
  // Unread is generated by the other PERSON. An agent's posts land in the right
  // conversation (scenario 1 proves that), but this instance owns that agent and
  // the server does not count an owned agent's messages as unread for its owner.
  const newRoot = await say('A separate question about the release notes.')
  const newInA = await say('One more thing about the lockfile.', { thread_id: threadA })
  const newInB = await say('Keys rotated.', { thread_id: threadB })
  await page.waitForTimeout(1000)

  const state = async () => {
    const chan = (await api('GET', '/users/me/read-state')).find((s) => s.channel_id === room.id)
    const views = await api('GET', `/channels/${room.id}/threads?limit=50`)
    return {
      room: chan.unread_count,
      a: views.find((v) => v.thread.id === threadA)?.read_state.unread_count,
      b: views.find((v) => v.thread.id === threadB)?.read_state.unread_count,
    }
  }
  const unreadAtStart = await state()
  // Reading the room clears its roots and leaves the conversations alone.
  await page.goto(`${base}/c/${room.id}`, { waitUntil: 'domcontentloaded' })
  await page.locator('.room .msg').first().waitFor()
  await page.waitForTimeout(1200)
  const afterRoomRead = await state()
  // Reading A clears A only.
  await page.goto(`${base}/c/${room.id}/t/${threadA}`, { waitUntil: 'domcontentloaded' })
  await page.locator('[data-thread-panel]').waitFor()
  await page.waitForTimeout(1200)
  const afterAread = await state()
  record('separateUnread', { unreadAtStart, afterRoomRead, afterAread, newRoot: newRoot.id, newInA: newInA.id, newInB: newInB.id })
  assert.ok(unreadAtStart.a >= 1 && unreadAtStart.b >= 1, 'the fixture did not leave both conversations unread')
  assert.equal(afterRoomRead.a, unreadAtStart.a, 'reading the room read a conversation')
  assert.equal(afterRoomRead.b, unreadAtStart.b, 'reading the room read another conversation')
  assert.equal(afterAread.a, 0, 'opening a conversation did not read it')
  assert.equal(afterAread.b, unreadAtStart.b, 'reading one conversation read another')

  // Unfollow, then a passive open, stays unfollowed.
  await page.locator('[data-thread-panel] button:has-text("Unfollow")').click()
  await page.waitForTimeout(600)
  await page.goto(`${base}/c/${room.id}`, { waitUntil: 'domcontentloaded' })
  await page.locator('.room .msg').first().waitFor()
  await page.goto(`${base}/c/${room.id}/t/${threadA}`, { waitUntil: 'domcontentloaded' })
  await page.locator('[data-thread-panel]').waitFor()
  await page.waitForTimeout(900)
  const following = (await api('GET', `/threads/${threadA}`)).read_state.following
  record('unfollowSurvivesOpening', { following })
  assert.equal(following, false, 'opening a conversation silently re-followed it')

  // Resolved WITH unread still outstanding. The realistic way that happens is
  // somebody else resolving it while your replies are unread — opening it
  // yourself to resolve it would have read it on the way in.
  await api('PATCH', `/threads/${threadB}`, { resolved: true }, mira.token)
  await page.waitForTimeout(900)
  const openStrip = await api('GET', `/channels/${room.id}/threads?limit=50&resolved=false`)
  const unreadAny = await api('GET', `/channels/${room.id}/threads?limit=50&unread_only=true`)
  await page.goto(`${base}/c/${room.id}`, { waitUntil: 'domcontentloaded' })
  await page.locator('.room .msg').first().waitFor()
  await page.waitForTimeout(700)
  const stripChips = await page.locator('[data-thread-strip] .chip').evaluateAll((els) => els.map((e) => e.dataset.thread))
  // The Unread threads action must reach it even though the open strip cannot.
  await page.locator('[data-thread-strip] button:has-text("Unread threads")').click()
  await page.waitForTimeout(900)
  const unreadChips = await page.locator('[data-thread-strip] .chip').evaluateAll((els) => els.map((e) => e.dataset.thread))
  record('resolvedStillReachable', {
    inOpenList: openStrip.some((v) => v.thread.id === threadB),
    inUnreadList: unreadAny.some((v) => v.thread.id === threadB),
    openStripChips: stripChips, unreadActionChips: unreadChips,
  })
  assert.equal(openStrip.some((v) => v.thread.id === threadB), false, 'a resolved conversation stayed in the open list')
  assert.ok(unreadAny.some((v) => v.thread.id === threadB), 'an unread resolved conversation became unreachable')
  assert.ok(!stripChips.includes(threadB), 'the open strip still showed a resolved conversation')
  assert.ok(unreadChips.includes(threadB), 'the Unread threads action did not reach the resolved conversation')

  // The resolve, rename and reopen controls themselves, on the conversation this
  // account has actually read.
  await openConversation(threadA)
  await page.locator('[data-thread-panel] button:has-text("Resolve")').click()
  await page.waitForTimeout(900)
  record('resolveControls', {
    panelStillOpen: await page.locator('[data-thread-panel]').count(),
    reopenOffered: await page.locator('[data-thread-panel] button:has-text("Reopen")').count(),
    composerLocked: await page.locator('[data-thread-panel] .locked').count(),
    sendDisabled: await page.locator('[data-thread-panel] .sendbtn').isDisabled(),
  })
  assert.equal(await page.locator('[data-thread-panel]').count(), 1, 'resolving closed the panel')
  assert.equal(await page.locator('[data-thread-panel] button:has-text("Reopen")').count(), 1, 'Reopen was not offered')
  assert.equal(await page.locator('[data-thread-panel] .locked').count(), 1, 'a resolved conversation still accepts new messages')

  await page.locator('[data-thread-panel] button:has-text("Rename")').click()
  await page.locator('[data-thread-panel] input[aria-label="Conversation title"]').fill('Lockfile release')
  await page.keyboard.press('Enter')
  await page.waitForTimeout(700)
  await page.locator('[data-thread-panel] button:has-text("Reopen")').click()
  await page.waitForTimeout(700)
  const afterReopen = (await api('GET', `/threads/${threadA}`)).thread
  record('renameAndReopen', { title: afterReopen.title, resolved: afterReopen.resolved_at ?? null })
  assert.equal(afterReopen.title, 'Lockfile release', 'rename did not stick')
  assert.equal(afterReopen.resolved_at ?? null, null, 'reopen did not stick')

  // Mark all read, crossed by a reply that arrives after the captured tail.
  await api('POST', `/channels/${room.id}/messages`, { content: 'something new in the room' }, mira.token)
  await page.waitForTimeout(800)
  await page.goto(`${base}/inbox`, { waitUntil: 'domcontentloaded' })
  await page.waitForTimeout(1200)
  // Genuinely CROSSING it: the tail capture is held, the reply is posted while
  // it is held, then it is released. Posting first and clicking afterwards would
  // simply have included the reply in the captured tail.
  let releaseTail
  const tailHeld = new Promise((r) => { releaseTail = r })
  let crossing
  await page.route(`**/channels/${room.id}/messages?limit=1`, async (route) => {
    const response = await route.fetch()
    crossing = await say('arriving across the mark-all', { thread_id: threadA })
    await tailHeld
    await route.fulfill({ response })
  })
  await page.locator('button:has-text("Mark all read")').click().catch(() => {})
  await page.waitForTimeout(1200)
  releaseTail()
  await page.waitForTimeout(1800)
  await page.unroute(`**/channels/${room.id}/messages?limit=1`)
  const afterMarkAll = (await api('GET', `/threads/${threadA}`)).read_state
  record('markAllCrossedByReply', {
    crossing: crossing?.id, lastRead: afterMarkAll.last_read_id, unread: afterMarkAll.unread_count,
  })
  assert.ok(crossing, 'the crossing reply was never posted')
  assert.ok(afterMarkAll.last_read_id < crossing.id,
    'Mark all read acknowledged a reply that arrived after its captured tail')

  // ===================== 4. Old targets and deep links ======================
  // A reply well outside the latest page, reached by URL and by search.
  for (let i = 0; i < 60; i++) await asAgent(`filler ${i + 1}`, { task_id: taskX })
  await page.waitForTimeout(400)
  await page.waitForTimeout(800)
  const target = repliesA[1]
  await page.goto(`${base}/c/${room.id}/t/${threadA}?m=${target.id}`, { waitUntil: 'domcontentloaded' })
  await page.locator('[data-thread-panel]').waitFor()
  await page.waitForTimeout(1500)
  record('oldTargetByUrl', {
    target: target.id,
    revealed: await page.locator(`[data-thread-panel] #t-${target.id}`).count(),
    panelThread: new URL(page.url()).pathname.split('/t/')[1],
  })
  assert.equal(await page.locator(`[data-thread-panel] #t-${target.id}`).count(), 1,
    'a direct link to an old reply did not reveal it')

  // A direct link with no history behind it falls back to its room.
  await page.locator('[data-thread-panel] button[aria-label="Close conversation"], [data-thread-panel] button[aria-label="Back to room"]').first().click()
  await page.waitForTimeout(700)
  record('directLinkFallback', { url: new URL(page.url()).pathname })
  assert.equal(new URL(page.url()).pathname, `/c/${room.id}`, 'closing a directly-linked conversation left the app')

  // ===================== 6. The cases the reviews said would dead-end ========

  // A resolved conversation, opened COLD from a saved URL on a fresh client.
  // Nothing has listed it: the open strip never mentions resolved threads, so
  // the route has to fetch its own metadata or this is a blank panel forever.
  await api('PATCH', `/threads/${threadB}`, { resolved: true }, mira.token)
  const cold = await context.newPage()
  const coldErrors = []
  cold.on('pageerror', (e) => coldErrors.push(String(e)))
  await cold.goto(`${base}/c/${room.id}/t/${threadB}`, { waitUntil: 'domcontentloaded' })
  await cold.locator('[data-thread-panel]').waitFor({ timeout: 10000 })
  await cold.waitForTimeout(1200)
  const coldState = {
    panel: await cold.locator('[data-thread-panel]').count(),
    rootShown: await cold.locator(`[data-thread-panel] #t-${otherJob.id}`).count(),
    resolvedTag: await cold.locator('[data-thread-panel] .tag').count(),
    reopen: await cold.locator('[data-thread-panel] button:has-text("Reopen")').count(),
    errors: coldErrors.length,
  }
  record('coldResolvedUrl', coldState)
  assert.equal(coldState.panel, 1, 'a reloaded resolved-thread URL did not open its panel')
  assert.equal(coldState.rootShown, 1, 'the cold panel never showed the message it is about')
  assert.equal(coldState.reopen, 1, 'the cold panel did not offer Reopen')
  await api('PATCH', `/threads/${threadB}`, { resolved: false }, mira.token)

  // Browseable resolved history: a READ resolved conversation, found through the
  // strip rather than through unread.
  await api('PATCH', `/threads/${threadB}`, { resolved: true }, mira.token)
  await cold.locator('[data-thread-strip] button:has-text("Resolved")').click()
  await cold.waitForTimeout(1200)
  const historyChips = await cold.locator('[data-thread-strip] .chip').evaluateAll((els) => els.map((e) => e.dataset.thread))
  record('resolvedHistory', { chips: historyChips.filter(Boolean) })
  assert.ok(historyChips.includes(threadB), 'resolved history could not reach a resolved conversation')

  // The unread list must not answer from a stale EMPTY result. Everything is
  // acknowledged first, so the list is genuinely empty when it is opened; a
  // cached empty answer would then survive the new reply below.
  await api('PUT', `/channels/${room.id}/read`,
    { message_id: (await api('GET', `/channels/${room.id}/messages?limit=1`))[0].id })
  await cold.waitForTimeout(800)
  await cold.locator('[data-thread-strip] button:has-text("Unread threads")').click()
  await cold.waitForTimeout(1200)
  const unreadBefore = await cold.locator('[data-thread-strip] .chip').evaluateAll((els) => els.map((e) => e.dataset.thread))
  const freshReply = await say('something new in the open conversation', { thread_id: threadA })
  await cold.waitForTimeout(1200)
  await cold.locator('[data-thread-strip] button:has-text("Open")').click()
  await cold.locator('[data-thread-strip] button:has-text("Unread threads")').click()
  await cold.waitForTimeout(1500)
  const unreadAfter = await cold.locator('[data-thread-strip] .chip').evaluateAll((els) => els.map((e) => e.dataset.thread))
  record('unreadListRefreshes', { before: unreadBefore.filter(Boolean), after: unreadAfter.filter(Boolean), reply: freshReply.id })
  assert.deepEqual(unreadBefore.filter(Boolean), [], 'the fixture did not start from an empty unread list')
  assert.ok(unreadAfter.includes(threadA), 'the unread list answered from its cached empty result')
  await cold.close()

  // ===================== 7. Live changes reach every displayed copy =========
  await page.goto(`${base}/c/${room.id}`, { waitUntil: 'domcontentloaded' })
  await page.locator('.room .msg').first().waitFor()
  await openConversation(threadA)
  // A reply that is actually on screen: the conversation has since grown past
  // one page, so an early reply would simply not be rendered and the assertion
  // would pass or fail for the wrong reason.
  const tailReplies = await api('GET', `/threads/${threadA}/messages?limit=5`)
  const replyInPanel = tailReplies.at(-1)
  await page.locator(`[data-thread-panel] #t-${replyInPanel.id}`).waitFor({ timeout: 10000 })
  // Somebody else reacts to a reply. It has to appear in the panel.
  await api('PUT', `/messages/${replyInPanel.id}/reactions`, { emoji: '👍' }, mira.token)
  await page.waitForTimeout(1500)
  record('reactionReachesPanel', {
    reply: replyInPanel.id,
    shown: await page.locator(`[data-thread-panel] #t-${replyInPanel.id} .rx`).count(),
  })
  assert.ok(await page.locator(`[data-thread-panel] #t-${replyInPanel.id} .rx`).count() >= 1,
    'a reaction on a reply never reached the conversation panel')

  // A deletion that crosses a reply-page refresh must not be undone by it.
  const doomed = await say('this reply gets deleted mid-refresh', { thread_id: threadA })
  await page.waitForTimeout(900)
  let releasePage
  const pageHeld = new Promise((r) => { releasePage = r })
  await page.route(`**/threads/${threadA}/messages**`, async (route) => {
    const response = await route.fetch()          // snapshot taken while it exists
    await pageHeld
    await route.fulfill({ response })
  })
  // Force a refresh, then delete while its snapshot is held.
  await page.locator('[data-thread-panel] button:has-text("Rename")').click()
  await page.keyboard.press('Escape')
  await page.evaluate(() => history.replaceState(history.state, '', location.href))
  await api('DELETE', `/messages/${doomed.id}`, undefined, mira.token)
  await page.waitForTimeout(1200)
  releasePage()
  await page.waitForTimeout(1200)
  // Scope, stated in the evidence itself: this proves ONE deletion of a reply
  // that was already in the latest cache. It does not prove window copies,
  // invalidation after a confirmed removal, or reaction repair on reconnect.
  record('deletionSurvivesRefresh', {
    proves: 'one deletion of an already-cached tail reply crossing a held page refresh',
    doesNotProve: ['window/target copies', 'confirmed-removal invalidation', 'reconnect reaction repair'],
    deleted: doomed.id,
    stillShown: await page.locator(`[data-thread-panel] #t-${doomed.id}`).count(),
  })
  assert.equal(await page.locator(`[data-thread-panel] #t-${doomed.id}`).count(), 0,
    'a held page refresh resurrected a deleted reply')
  await page.unroute(`**/threads/${threadA}/messages**`)

  // ===================== 8. Drops belong to the pane they land on ===========
  scratch = await mkdtemp('/tmp/den-thread-ui-')
  const png = `${scratch}/one.png`
  await writeFile(png, Buffer.from(
    'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNk+M9QDwADhgGAWjR9awAAAABJRU5ErkJggg==',
    'base64'))
  await page.locator('[data-thread-panel] input[type=file]').setInputFiles(png)
  await page.waitForFunction(() => document.querySelectorAll('[data-thread-panel] .fname').length === 1, null, { timeout: 10000 })
  const paneFiles = {
    panel: await page.locator('[data-thread-panel] .fname').count(),
    room: await page.locator('.room .fname').count(),
  }
  record('filesBelongToTheirPane', paneFiles)
  assert.equal(paneFiles.panel, 1, 'the conversation lost its own file')
  assert.equal(paneFiles.room, 0, "a conversation's file was staged in the room")

  // And the upload IDs it sends are exactly its own.
  const uploads = []
  await page.route(`**/channels/${room.id}/messages`, async (route) => {
    if (route.request().method() === 'POST') uploads.push(JSON.parse(route.request().postData() || '{}'))
    return route.fulfill({ response: await route.fetch() })
  })
  await page.locator('[data-thread-panel] textarea').fill('')
  await page.locator('[data-thread-panel] textarea').type('with its own attachment')
  // The composer refuses to send while an upload is still going, so wait for it
  // rather than pressing Enter into a no-op.
  await page.waitForFunction(
    () => !document.querySelector('[data-thread-panel] .sendbtn')?.disabled, null, { timeout: 15000 })
  await page.keyboard.press('Enter')
  await page.waitForTimeout(1500)
  record('paneUploadIds', { sent: uploads[0] })
  assert.equal(uploads[0]?.thread_id, threadA, 'the attachment was sent to the wrong conversation')
  assert.equal(uploads[0]?.upload_ids?.length, 1, 'the conversation did not send its own upload')
  await page.unroute(`**/channels/${room.id}/messages`)

  // ===================== 9. History means what its label says ==============
  // Wide, where both panes are visible: room -> A -> B by the strip. The switch
  // REPLACES A's entry, so one Back from B is the room, not A.
  await page.goto(`${base}/c/${room.id}`, { waitUntil: 'domcontentloaded' })
  await page.locator('.room .msg').first().waitFor()
  await openConversation(threadA)
  await openConversation(threadB)
  const atB = new URL(page.url()).pathname
  await backToRoom()
  const afterBack = new URL(page.url()).pathname
  await page.goForward()
  await page.waitForTimeout(700)
  const afterForward = new URL(page.url()).pathname
  await page.goBack()
  await page.waitForTimeout(700)
  const afterBackAgain = new URL(page.url()).pathname
  record('historyParent', { atB, afterBack, afterForward, afterBackAgain })
  assert.ok(atB.includes(`/t/${threadB}`), 'the strip switch did not open B')
  assert.equal(afterBack, `/c/${room.id}`, 'Back to room did not return to the room')
  assert.ok(afterForward.includes(`/t/${threadB}`), 'forward did not return to the conversation it left')
  assert.equal(afterBackAgain, `/c/${room.id}`, 'back after forward did not return to the room')

  // Narrow, where the conversation takes the whole width: the room and its strip
  // are hidden, so Back to room is the way out, and focus returns to the opener.
  await page.setViewportSize(VIEWPORTS.narrow)
  await page.waitForTimeout(500)
  await page.goto(`${base}/c/${room.id}`, { waitUntil: 'domcontentloaded' })
  await page.locator('.room .msg').first().waitFor()
  await openConversation(threadA)
  const narrowState = {
    roomHidden: await page.locator('.room.stowed').count(),
    roomInert: await page.locator('.room[inert]').count(),
    backLabel: await page.locator('[data-thread-panel] button[aria-label="Back to room"]').count(),
  }
  await backToRoom()
  await page.waitForTimeout(700)
  const focused = await page.evaluate(() => {
    const el = document.activeElement
    return { tag: el?.tagName, id: el?.closest('[id]')?.id ?? '', inHidden: !!el?.closest('.room.stowed') }
  })
  record('narrowReturn', { ...narrowState, url: new URL(page.url()).pathname, focused })
  assert.equal(narrowState.roomHidden, 1, 'the room stayed visible behind a full-width conversation')
  assert.equal(narrowState.roomInert, 1, 'the hidden room was still focusable')
  assert.equal(narrowState.backLabel, 1, 'the narrow panel offered no way back')
  assert.equal(new URL(page.url()).pathname, `/c/${room.id}`, 'Back to room did not return to the room')
  assert.equal(focused.inHidden, false, 'focus was left inside a hidden pane')
  await page.setViewportSize(VIEWPORTS.landscape)
  await page.waitForTimeout(400)

  // ===================== 5. A real canvas inside a conversation =============
  await page.goto(`${base}/c/${room.id}/t/${threadA}`, { waitUntil: 'domcontentloaded' })
  await page.locator('[data-thread-panel]').waitFor()
  await type('[data-thread-panel]', '/canvas Lockfile sketch')
  await page.keyboard.press('Enter')
  await page.waitForTimeout(2500)
  const threadAfterCanvas = await api('GET', `/threads/${threadA}/messages?limit=100`)
  const canvasMsg = threadAfterCanvas.find((m) => (m.objects || []).some((o) => o.kind === 'canvas'))
  const roomAfterCanvas = await api('GET', `/channels/${room.id}/messages?limit=50&roots_only=true`)
  record('canvasInConversation', {
    inThread: !!canvasMsg,
    objectId: canvasMsg?.objects?.[0]?.id ?? null,
    leakedToRoom: roomAfterCanvas.some((m) => (m.objects || []).some((o) => o.kind === 'canvas')),
    cardRendered: await page.locator('[data-thread-panel] .msg').count(),
  })
  assert.ok(canvasMsg, 'the canvas card did not land in the conversation')
  assert.equal(roomAfterCanvas.some((m) => (m.objects || []).some((o) => o.kind === 'canvas')), false,
    'a canvas started in a conversation appeared in the room')

  // ===================== 9b. Search and quote reach the real destination ====
  // These go through goToMessage, whose owner check rejects the exported store
  // PROXY. A helper test cannot see that: the callers are Svelte components, so
  // the only proof is clicking them.
  const deepReply = repliesA[3] ?? repliesA.at(-1)
  await page.goto(`${base}/find?q=${encodeURIComponent('lockfile')}&in=${room.id}`, { waitUntil: 'domcontentloaded' })
  await page.locator('.result, [data-result], .line').first().waitFor({ timeout: 10000 }).catch(() => {})
  const hit = page.locator(`text=${'lockfile'}`).first()
  await hit.click({ timeout: 10000 }).catch(() => {})
  await page.waitForTimeout(1500)
  const afterSearch = new URL(page.url())
  record('searchResultNavigates', {
    path: afterSearch.pathname, target: afterSearch.searchParams.get('m'),
    panel: await page.locator('[data-thread-panel]').count(),
  })
  assert.notEqual(afterSearch.pathname, '/find', 'clicking a search result did not navigate anywhere')
  assert.ok(afterSearch.searchParams.get('m'), 'a search result navigated without revealing its message')

  // A quoted reference inside a conversation. One is posted now so it is in the
  // visible tail: the conversation has long since grown past a page, and an
  // early reply would simply not be rendered.
  const quotedParent = (await api('GET', `/threads/${threadA}/messages?limit=2`))[0]
  await say('answering that specific point', { thread_id: threadA, reply_to: quotedParent.id })
  await openConversation(threadA)
  await page.waitForTimeout(1500)
  const quoted = page.locator('[data-thread-panel] .reply-ref').last()
  if (await quoted.count()) {
    const before = new URL(page.url()).toString()
    await quoted.click()
    await page.waitForTimeout(1200)
    record('quoteReferenceNavigates', { before, after: page.url(), changed: page.url() !== before })
    assert.notEqual(page.url(), before, 'clicking a quoted reference did nothing')
  } else {
    record('quoteReferenceNavigates', { ran: false, why: 'no quoted reference rendered in this fixture' })
  }

  // ===================== 10. A real terminal inside a conversation ==========
  // Only when a host is actually connected. Absence is reported, never faked.
  const hosts = await api('GET', '/hosts').catch(() => [])
  const host = hosts.find((h) => h.online)
  if (!host) {
    record('terminalInConversation', { ran: false, why: 'no host was connected to this isolated server' })
  } else {
    await openConversation(threadA)
    await page.locator('[data-thread-panel] textarea').fill(`/terminal ${host.name}`)
    await page.keyboard.press('Enter')
    await page.waitForTimeout(7000)
    const withTerminal = await api('GET', `/threads/${threadA}/messages?limit=100`)
    const card = withTerminal.find((m) => (m.objects || []).some((o) => o.kind === 'terminal'))
    const rootsNow = await api('GET', `/channels/${room.id}/messages?limit=50&roots_only=true`)
    // Type into the real shell and read its real output back.
    const marker = `den-${Date.now().toString(36)}`
    await page.locator('[data-thread-panel] .xterm, .object-slot .xterm').first().click({ timeout: 15000 }).catch(() => {})
    await page.keyboard.type(`echo ${marker}`)
    await page.keyboard.press('Enter')
    await page.waitForTimeout(4000)
    const echoed = (await page.evaluate(() => document.body.innerText)).includes(marker)
    record('terminalInConversation', {
      ran: true, host: host.name, inThread: !!card, objectId: card?.objects?.[0]?.id ?? null,
      leakedToRoom: rootsNow.some((m) => (m.objects || []).some((o) => o.kind === 'terminal')),
      marker, observedShellOutput: echoed,
    })
    assert.ok(card, 'the terminal card did not land in the conversation')
    assert.equal(rootsNow.some((m) => (m.objects || []).some((o) => o.kind === 'terminal')), false,
      'a terminal opened from a conversation appeared in the room')
    assert.ok(echoed, `the shell never echoed ${marker}; no real PTY output observed`)

    // Resolving ends the conversation, not the session: the card stays usable
    // and new content is refused until it is explicitly reopened.
    await api('PATCH', `/threads/${threadA}`, { resolved: true })
    const refused = await fetch(`${base}/channels/${room.id}/messages`, {
      method: 'POST',
      headers: { 'content-type': 'application/json', authorization: `Bearer ${token}` },
      body: JSON.stringify({ content: 'should be refused', thread_id: threadA }),
    })
    const objectStill = await fetch(`${base}/objects/${card.objects[0].id}`, { headers: { authorization: `Bearer ${token}` } })
    await api('PATCH', `/threads/${threadA}`, { resolved: false })
    record('resolvePreservesSession', {
      newMessageStatus: refused.status, objectStatus: objectStill.status,
    })
    assert.equal(refused.status, 409, 'a resolved conversation accepted a new message')
    assert.equal(objectStill.status, 200, 'resolving broke an existing terminal card')
  }

  results.pageErrors = pageErrors
  assert.deepEqual(pageErrors.filter((e) => !e.expected), [], 'the page reported unexpected errors')
  results.ok = true
} finally {
  await browser?.close()
  if (scratch) await rm(scratch, { recursive: true, force: true })
  await writeFile(`${shots}results.json`, JSON.stringify(results, null, 2) + '\n')
  console.log(`\n${phase}: ${results.ok ? 'PASS' : 'FAIL'} — evidence in docs/shots/pr/thread-web-ui/${phase}/`)
}
