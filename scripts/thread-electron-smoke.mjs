// Thread activation in real Electron: a real X11 window, resized natively, not
// a browser viewport emulation. Uses its own display, user-data directory and
// throwaway account — never the private acceptance profile.
//
// It LAUNCHES Electron itself, through Playwright's Electron launcher, rather
// than attaching to an already-running instance over CDP. That is not a
// preference: the zoom step below needs the MAIN process, because Electron's
// zoom lives on webContents and CDP cannot reach it. Attaching over CDP is what
// produced the earlier zoom result that measured nothing.
//
// DISPLAY must point at the X server it should open on, and the disposable
// Secret Service must already own org.freedesktop.secrets on this bus.
import { _electron as electron } from 'playwright-core'
import { execFileSync } from 'node:child_process'
import { mkdir, writeFile } from 'node:fs/promises'
import assert from 'node:assert/strict'

const base = process.env.DEN_SMOKE_URL
const display = process.env.DISPLAY || ':78'
const shots = new URL('../docs/shots/pr/thread-web-ui/electron/', import.meta.url).pathname
await mkdir(shots, { recursive: true })

const x = (...args) => execFileSync('xdotool', args.map(String), { env: { ...process.env, DISPLAY: display } }).toString().trim()
/// Best-effort: this display has no window manager, so activation is a no-op
/// here. Sizing and moving work regardless, and focus comes through CDP.
const xTry = (...args) => { try { return x(...args) } catch { return '' } }
const sleep = (ms) => new Promise((r) => setTimeout(r, ms))
const results = { display, base, steps: {} }
const record = (name, value) => { results.steps[name] = value; console.log(`  ${name}:`, JSON.stringify(value)) }

const api = async (method, path, body, as) => {
  const r = await fetch(base + path, {
    method,
    headers: { 'content-type': 'application/json', ...(as ? { authorization: `Bearer ${as}` } : {}) },
    body: body ? JSON.stringify(body) : undefined,
  })
  const payload = r.status === 204 ? '' : await r.text()
  assert(r.ok, `${method} ${path}: ${r.status} ${payload}`)
  return payload ? JSON.parse(payload) : null
}

const app = await electron.launch({
  executablePath: process.env.DEN_ELECTRON_BIN,
  cwd: process.env.DEN_APP_DIR,
  args: ['.', '--password-store=gnome-libsecret', '--no-sandbox'],
  env: { ...process.env, DEN_DESKTOP_DATA: process.env.DEN_PROFILE, DEN_DESKTOP_URL: base },
})
const page = await app.firstWindow()
page.setDefaultTimeout(30000)
const errors = []
page.on('pageerror', (e) => errors.push(String(e)))

/// Exactly one window, asserted rather than assumed. Every measurement below
/// mixes main-process values (bounds, zoom factor) with renderer values, and a
/// second window would let those describe different things.
const windowCount = await app.evaluate(({ BrowserWindow }) => BrowserWindow.getAllWindows().length)
assert.equal(windowCount, 1, `expected one Electron window, found ${windowCount}`)

/// The X11 window can be mapped slightly after the renderer is ready.
let windowId = process.env.DEN_ELECTRON_WINDOW || ''
for (let i = 0; !windowId && i < 80; i++) {
  const found = xTry('search', '--onlyvisible', '--name', '.').split('\n').filter(Boolean)
  if (found.length) { windowId = found[0]; break }
  await new Promise((r) => setTimeout(r, 150))
}
assert.ok(windowId, 'BLOCKED: the Electron window never appeared on this display')
/// Resize the native X11 client window and REQUIRE that it took effect. A size
/// that never arrived is a blocked run, not a passing one: returning whatever
/// the window happened to be would let every later assertion describe a window
/// nobody asked for.
async function resize(width, height) {
  x('windowsize', windowId, width, height)
  xTry('windowmove', windowId, 20, 20)
  xTry('windowactivate', windowId)
  for (let i = 0; i < 80; i++) {
    const got = await page.evaluate(() => [innerWidth, innerHeight])
    if (Math.abs(got[0] - width) <= 40 && Math.abs(got[1] - height) <= 60) return got
    await sleep(150)
  }
  const got = await page.evaluate(() => [innerWidth, innerHeight])
  const geometry = xTry('getwindowgeometry', windowId)
  throw new Error(
    `BLOCKED: the native window never reached ${width}x${height}; it is ${got[0]}x${got[1]} (${geometry}). ` +
    'Not reporting the resulting layout as a pass.')
}

try {
  // --- Sign in through the real native login -------------------------------
  const token = (await api('POST', '/auth/login',
    { username: 'nicholas', password: process.env.DEN_SMOKE_PASSWORD }, '')).token
  // This run builds its OWN fixture instead of reusing the newest workshop-*
  // channel. Reusing one made the run depend on whatever earlier runs left
  // behind: the conversation it picked had grown to 73 replies, so every run
  // tested something different and the keyboard walk eventually could not
  // finish at all. A channel per run is the same UI on known content.
  const tag = Date.now().toString(36)
  const room = await api('POST', '/channels', { name: `electron-${tag}`, kind: 'text', position: 0 }, token)
  const root = await api('POST', `/channels/${room.id}/messages`,
    { content: 'The deploy failed again on run two.' }, token)
  for (const reply of ['I can reproduce it. Looking now.', 'Found it: the lockfile is not released.'])
    await api('POST', `/channels/${room.id}/messages`, { content: reply, reply_to: root.id }, token)
  // A second root, so the room behind the panel is not a single message.
  await api('POST', `/channels/${room.id}/messages`, { content: 'Unrelated: rotating the backup keys.' }, token)
  const views = await api('GET', `/channels/${room.id}/threads?limit=10`, undefined, token)
  const open = views.find((v) => !v.thread.resolved_at) ?? views[0]
  assert.ok(open, 'the fixture conversation was not created; the rest of this run would be meaningless')

  await resize(1440, 900)
  // Point this profile at the isolated server BEFORE any credential is typed.
  // A fresh Electron profile defaults to the production origin, and a login
  // attempt from here must never be aimed at it.
  const pointed = await page.evaluate((o) => {
    if (localStorage.getItem('den.native.origin') === o) return false
    localStorage.setItem('den.native.origin', o)
    return true
  }, base)
  if (pointed) { await page.reload({ waitUntil: 'domcontentloaded' }); await sleep(2500) }
  // localStorage is what the app INTENDS to use. What matters is where a request
  // actually goes: an earlier run of this smoke typed into a profile still
  // pointed at the production origin, and only a real request would have shown
  // it. So make one harmless unauthenticated request and read its actual URL.
  const seen = []
  const watch = (r) => seen.push(r.url())
  page.on('request', watch)
  await page.evaluate(() => fetch('/health').catch(() => {}))
  await sleep(800)
  page.off('request', watch)
  const health = seen.filter((u) => u.includes('/health'))
  record('requestOrigin', { observed: health, expected: base })
  assert.ok(health.length, 'no request was observed, so the origin is unverified')
  assert.ok(health.every((u) => u.startsWith(base)),
    `the app is talking to ${health.join(', ')}, not the isolated server; refusing to sign in`)
  if (await page.locator('input[autocomplete="username"]').count()) {
    await page.locator('input[autocomplete="username"]').fill('nicholas')
    await page.locator('input[type=password]').fill(process.env.DEN_SMOKE_PASSWORD)
    await page.locator('button[type=submit]').click()
  }
  await page.locator(`a[href="/c/${room.id}"]`).waitFor({ timeout: 30000 })
  await page.locator(`a[href="/c/${room.id}"]`).click()
  await page.locator('.room .msg').first().waitFor()
  await sleep(1200)
  record('signedIn', { room: room.id, thread: open.thread.id, native: await page.evaluate(() => !!window.__TAURI_INTERNALS__ || navigator.userAgent.includes('Electron')) })

  // --- Landscape: room and conversation side by side ------------------------
  const landscape = await resize(1440, 900)
  await page.locator('[data-thread-strip]').waitFor()
  await page.locator(`[data-thread-strip] .chip[data-thread="${open.thread.id}"]`).click()
  await page.locator('[data-thread-panel]').waitFor()
  await sleep(900)
  await page.locator('.room textarea').fill('room draft in electron')
  await page.locator('[data-thread-panel] textarea').fill('conversation draft in electron')
  await sleep(400)
  const wide = {
    size: landscape,
    roomVisible: await page.locator('.room:not(.stowed)').count(),
    panel: await page.locator('[data-thread-panel]').count(),
    controls: await page.locator('[data-thread-panel] button:has-text("Resolve")').isVisible(),
  }
  await page.screenshot({ path: `${shots}landscape.png` })
  record('landscape', wide)
  assert.equal(wide.panel, 1, 'no conversation panel in a wide Electron window')
  assert.equal(wide.roomVisible, 1, 'the room was hidden even though there was room for both')
  assert.ok(wide.controls, 'the Resolve control was not visible')

  // --- Portrait: the conversation takes the width, with a way back ----------
  const portrait = await resize(820, 1200)
  await sleep(900)
  const tall = {
    size: portrait,
    roomStowed: await page.locator('.room.stowed').count(),
    roomInert: await page.locator('.room[inert]').count(),
    back: await page.locator('[data-thread-panel] button[aria-label="Back to room"]').isVisible(),
    // Nothing clipped: the send and the return must be inside the window.
    inWindow: await page.evaluate(() => {
      const els = [...document.querySelectorAll('[data-thread-panel] .sendbtn, [data-thread-panel] .head button')]
      return els.every((e) => { const r = e.getBoundingClientRect(); return r.right <= innerWidth + 1 && r.bottom <= innerHeight + 1 })
    }),
    draft: await page.locator('[data-thread-panel] textarea').inputValue(),
  }
  await page.screenshot({ path: `${shots}portrait.png` })
  record('portrait', tall)
  assert.equal(tall.roomStowed, 1, 'the room stayed beside the conversation in a portrait window')
  assert.equal(tall.roomInert, 1, 'the hidden room was still focusable')
  assert.ok(tall.back, 'no way back to the room in portrait')
  assert.ok(tall.inWindow, 'a control was clipped outside the portrait window')
  assert.equal(tall.draft, 'conversation draft in electron', 'the conversation draft was lost on resize')

  // --- Narrow window --------------------------------------------------------
  const narrow = await resize(700, 900)
  await sleep(900)
  const tight = {
    size: narrow,
    roomStowed: await page.locator('.room.stowed').count(),
    back: await page.locator('[data-thread-panel] button[aria-label="Back to room"]').isVisible(),
    inWindow: await page.evaluate(() => [...document.querySelectorAll('[data-thread-panel] .sendbtn, [data-thread-panel] .head button')]
      .every((e) => { const r = e.getBoundingClientRect(); return r.right <= innerWidth + 1 && r.left >= -1 })),
    draft: await page.locator('[data-thread-panel] textarea').inputValue(),
  }
  await page.screenshot({ path: `${shots}narrow.png` })
  record('narrow', tight)
  assert.equal(tight.roomStowed, 1, 'the room stayed beside the conversation in a narrow window')
  assert.ok(tight.back, 'no way back to the room in a narrow window')
  assert.ok(tight.inWindow, 'a control was clipped outside the narrow window')
  assert.equal(tight.draft, 'conversation draft in electron', 'the narrow window lost the draft')

  // --- Keyboard: Tab to the way back and press it, while NARROW -------------
  // This is the case that matters: the room is hidden, so the control being
  // tabbed to is the only way out. In a wide window the same control is a
  // Close, and reaching it proves less.
  // Driven, not asserted about: focus() plus a tag check would prove nothing
  // about whether anyone can reach this control with a keyboard.
  await page.locator('[data-thread-panel] textarea').focus()
  const walk = []
  let landed = null
  // Bounded with room to spare. Tabbing BACKWARDS from the composer walks up
  // through the messages first, so the budget has to cover the fixture above
  // (roughly four stops per message) and not much more. The distance actually
  // walked is recorded, so this shows up as a rising number before it ever
  // becomes a failure.
  for (let i = 0; i < 40 && !landed; i++) {
    await page.keyboard.press('Shift+Tab')
    const at = await page.evaluate(() => {
      const el = document.activeElement
      return {
        label: el?.getAttribute('aria-label') ?? el?.textContent?.trim().slice(0, 24) ?? '',
        inPanel: !!el?.closest('[data-thread-panel]'),
        inHidden: !!el?.closest('.room.stowed'),
      }
    })
    walk.push(at.label)
    assert.equal(at.inHidden, false, 'tabbing reached a control inside the hidden room')
    if (at.inPanel && /back to room/i.test(at.label)) landed = at
  }
  const urlBefore = new URL(page.url()).pathname
  if (landed) await page.keyboard.press('Enter')
  await sleep(900)
  record('keyboardReturn', {
    stops: walk.length, landed: landed?.label ?? null, walk,
    urlBefore, urlAfter: new URL(page.url()).pathname,
  })
  assert.ok(landed, `Back to room was not reachable by keyboard; tab order was ${walk.join(' -> ')}`)
  assert.notEqual(new URL(page.url()).pathname, urlBefore, 'pressing Enter on Back to room did nothing')

  // That keypress closed the panel, which the remaining checks need open.
  await page.locator(`[data-thread-strip] .chip[data-thread="${open.thread.id}"]`).click()
  await page.locator('[data-thread-panel]').waitFor()
  await sleep(700)

  // --- 200% zoom, actually measured ----------------------------------------
  // Electron's zoom is a webContents property, reachable only from the MAIN
  // process. The earlier version of this step drove Emulation.setPageScaleFactor
  // over CDP and asserted its own boolean; it recorded DPR 1 and an unchanged
  // width while claiming 200%. So this asserts the values CHANGED, and a call
  // that returns without changing anything is a failed run, not an applied one.
  //
  // The native window is deliberately NOT resized here: holding it still is what
  // makes the renderer's shrinking CSS width mean zoom rather than a resize.
  const zoomState = async () => ({
    zoomFactor: await app.evaluate(({ BrowserWindow }) =>
      BrowserWindow.getAllWindows()[0].webContents.getZoomFactor()),
    nativeBounds: await app.evaluate(({ BrowserWindow }) => BrowserWindow.getAllWindows()[0].getBounds()),
    renderer: await page.evaluate(() => ({
      innerWidth, innerHeight, devicePixelRatio, cssWidth: document.documentElement.clientWidth,
    })),
  })
  const zoomBefore = await zoomState()
  await page.screenshot({ path: `${shots}zoom-100.png` })
  const zoomReturned = await app.evaluate(({ BrowserWindow }) => {
    const w = BrowserWindow.getAllWindows()[0]
    w.webContents.setZoomFactor(2)
    return w.webContents.getZoomFactor()
  })
  await sleep(1200)
  const zoomAfter = await zoomState()
  await page.screenshot({ path: `${shots}zoom-200.png` })

  const zoomed = {
    setZoomFactorReturned: zoomReturned, before: zoomBefore, after: zoomAfter,
    panel: await page.locator('[data-thread-panel]').count(),
    back: await page.locator('[data-thread-panel] button[aria-label="Back to room"], [data-thread-panel] button[aria-label="Close conversation"]').first().isVisible(),
    draft: await page.locator('[data-thread-panel] textarea').inputValue(),
    // BOTH axes. An earlier version of this check tested left/right only, and a
    // header that wraps under zoom can push a control below the fold instead of
    // past the edge — just as unreachable, and it would have passed. These live
    // in a fixed header and a fixed composer, so neither should ever need
    // scrolling to reach.
    fits: await page.evaluate(() =>
      [...document.querySelectorAll('[data-thread-panel] .sendbtn, [data-thread-panel] .head button')]
        .map((e) => {
          const r = e.getBoundingClientRect()
          return {
            label: e.getAttribute('aria-label') || e.textContent.trim().slice(0, 20),
            inside: r.right <= innerWidth + 1 && r.left >= -1 && r.bottom <= innerHeight + 1 && r.top >= -1,
            x: [Math.round(r.left), Math.round(r.right)], y: [Math.round(r.top), Math.round(r.bottom)],
          }
        })),
  }
  // Controls fitting is not the whole story: message text must wrap rather than
  // run off the side. Per-element scrollWidth alone cannot see that, because an
  // ANCESTOR wider than the viewport inside an overflow:hidden container clips
  // content visually while every .text still fits its own box and the document
  // never scrolls sideways. So walk up and measure where the boxes actually are.
  zoomed.textOverflow = await page.evaluate(() => {
    const texts = [...document.querySelectorAll('[data-thread-panel] .text, .room .msg .text')]
    const overflowing = texts.filter((e) => e.scrollWidth > e.clientWidth + 1)
      .map((e) => ({ scroll: e.scrollWidth, client: e.clientWidth, text: e.textContent.slice(0, 40) }))
    const boxes = []
    for (const e of texts) {
      for (let n = e; n && n !== document.body; n = n.parentElement) {
        const r = n.getBoundingClientRect()
        if (r.right > innerWidth + 1 || r.left < -1) {
          boxes.push({ cls: n.className?.toString().slice(0, 30), left: Math.round(r.left), right: Math.round(r.right) })
          break
        }
      }
    }
    return {
      textsExamined: texts.length, overflowCount: overflowing.length, sample: overflowing.slice(0, 3),
      ancestorsOutsideViewport: boxes.length, ancestorSample: boxes.slice(0, 3),
      viewportWidth: innerWidth,
      documentScrollsSideways: document.documentElement.scrollWidth > document.documentElement.clientWidth + 1,
    }
  })
  record('zoom200', zoomed)

  assert.equal(zoomReturned, 2, 'setZoomFactor did not report 2')
  assert.equal(zoomAfter.zoomFactor, 2, 'getZoomFactor() is not 2 after the call')
  assert.deepEqual(zoomAfter.nativeBounds, zoomBefore.nativeBounds,
    'the native window changed size during the zoom; the comparison would be meaningless')
  assert.ok(zoomAfter.renderer.innerWidth < zoomBefore.renderer.innerWidth,
    `renderer CSS width did not shrink under zoom: ${zoomBefore.renderer.innerWidth} -> ${zoomAfter.renderer.innerWidth}`)
  assert.ok(zoomAfter.renderer.devicePixelRatio > zoomBefore.renderer.devicePixelRatio,
    `devicePixelRatio did not rise under zoom: ${zoomBefore.renderer.devicePixelRatio} -> ${zoomAfter.renderer.devicePixelRatio}`)
  // A zero that came from examining nothing is not evidence.
  assert.ok(zoomed.textOverflow.textsExamined > 0, 'no message text was examined; the check is vacuous')
  assert.equal(zoomed.textOverflow.ancestorsOutsideViewport, 0,
    `content extends outside the viewport at 2x: ${JSON.stringify(zoomed.textOverflow.ancestorSample)}`)
  assert.equal(zoomed.panel, 1, 'the conversation disappeared at 200%')
  assert.ok(zoomed.back, 'the way back was unreachable at 200%')
  assert.ok(zoomed.fits.length > 0, 'no panel controls were found; the fit check is vacuous')
  assert.deepEqual(zoomed.fits.filter((c) => !c.inside), [],
    'a control was outside the viewport at 200%')
  assert.equal(zoomed.draft, 'conversation draft in electron', 'the conversation draft did not survive zooming')

  await app.evaluate(({ BrowserWindow }) => BrowserWindow.getAllWindows()[0].webContents.setZoomFactor(1))
  await sleep(500)

  // --- Back to landscape: both drafts survive the round trip ----------------
  const back = await resize(1440, 900)
  await sleep(1000)
  const returned = {
    size: back,
    room: await page.locator('.room textarea').inputValue(),
    panelDraft: await page.locator('[data-thread-panel] textarea').inputValue(),
    roomVisible: await page.locator('.room:not(.stowed)').count(),
  }
  await page.screenshot({ path: `${shots}landscape-return.png` })
  record('backToLandscape', returned)
  assert.equal(returned.room, 'room draft in electron', 'the room draft did not survive the resize round trip')
  assert.equal(returned.panelDraft, 'conversation draft in electron', 'the conversation draft did not survive it')
  assert.equal(returned.roomVisible, 1, 'the room did not come back beside the conversation')

  results.errors = errors
  assert.deepEqual(errors, [], 'Electron reported page errors')
  results.ok = true
} finally {
  await writeFile(`${shots}results.json`, JSON.stringify(results, null, 2) + '\n')
  await app.close().catch(() => {})
  console.log(`\nelectron: ${results.ok ? 'PASS' : 'FAIL'} — evidence in docs/shots/pr/thread-web-ui/electron/`)
}
