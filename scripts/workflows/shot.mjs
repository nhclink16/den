// Screenshot one surface of Den for the visual-audit workflow.
// Usage: node scripts/workflows/shot.mjs --url <base> --path /settings/voice --out shot.png
//        [--w 1280] [--h 800] [--scheme dark] [--user nicholas] [--password ...]
// Prints a JSON line describing what it captured, including any console errors.
import { chromium } from 'playwright-core'

const arg = (name, fallback) => {
  const i = process.argv.indexOf(`--${name}`)
  return i === -1 ? fallback : process.argv[i + 1]
}
const base = arg('url', 'http://localhost:5173')
const target = arg('path', '/')
const out = arg('out', 'shot.png')
const width = Number(arg('w', 1280))
const height = Number(arg('h', 800))
const scheme = arg('scheme', 'dark')
const user = arg('user', 'nicholas')
const password = arg('password', '')

const browser = await chromium.launch({ executablePath: '/usr/bin/chromium', args: ['--no-sandbox'] })
const ctx = await browser.newContext({ viewport: { width, height }, colorScheme: scheme })
const page = await ctx.newPage()
const problems = []
page.on('pageerror', (e) => problems.push(`pageerror: ${e.message.slice(0, 200)}`))
page.on('console', (m) => {
  // A 401 on /users/me is the normal logged-out probe, not a defect.
  if (m.type() === 'error' && !m.text().includes('401')) problems.push(`console: ${m.text().slice(0, 200)}`)
})

try {
  await page.goto(`${base}/login`, { waitUntil: 'networkidle' })
  if (password && target !== '/login') {
    await page.fill('input[autocomplete=username]', user)
    await page.fill('input[type=password]', password)
    await page.click('button[type=submit]')
    await page.waitForTimeout(2200)
  }
  if (target !== '/login') {
    await page.goto(base + target, { waitUntil: 'networkidle' })
    await page.waitForTimeout(1200)
  }
  await page.screenshot({ path: out, fullPage: false })

  // Cheap objective signals the reviewer would otherwise have to eyeball.
  const facts = await page.evaluate(() => {
    const doc = document.documentElement
    const overflowing = [...document.querySelectorAll('*')]
      .filter((el) => el.getBoundingClientRect().right > window.innerWidth + 1)
      .slice(0, 8)
      .map((el) => `${el.tagName.toLowerCase()}.${(el.className || '').toString().split(' ')[0]}`)
    // Only count things a finger could actually hit. Message hover-tools sit in the DOM
    // at opacity 0 with pointer-events none; counting them made this list 87% noise and
    // taught reviewers to distrust the whole evidence block.
    const tiny = []
    let hiddenSmall = 0
    for (const el of document.querySelectorAll('button, a, input, [role=button]')) {
      const r = el.getBoundingClientRect()
      if (r.width === 0 || r.height === 0) continue
      if (r.width >= 32 && r.height >= 32) continue
      const cs = getComputedStyle(el)
      const reachable = cs.pointerEvents !== 'none' && cs.visibility !== 'hidden' && Number(cs.opacity) > 0.05
      if (!reachable) { hiddenSmall++; continue }
      const label = (el.textContent || el.getAttribute('aria-label') || el.getAttribute('placeholder') || '?').trim().slice(0, 24)
      // The hit area may be a padded wrapper, not the control itself.
      const pr = el.parentElement ? el.parentElement.getBoundingClientRect() : r
      const wrap = (pr.height > r.height + 4 || pr.width > r.width + 4)
        ? ` (hit area ${Math.round(pr.width)}x${Math.round(pr.height)})` : ''
      tiny.push(`${label} — ${Math.round(r.width)}x${Math.round(r.height)}${wrap}`)
    }
    // Prove scrollability by scrolling, rather than inferring it from scrollWidth.
    // body{overflow-x:hidden} does not stop the html element panning, and a reviewer
    // reading the CSS alone will reason its way to the wrong answer.
    const startX = window.scrollX
    window.scrollTo(9999, window.scrollY)
    const reachedX = window.scrollX
    window.scrollTo(startX, window.scrollY)
    const overshoot = doc.scrollWidth - doc.clientWidth
    return {
      horizontalOverflowPx: overshoot > 2 ? overshoot : 0,
      userCanActuallyPanSideways: reachedX > startX,
      panDistancePx: reachedX - startX,
      anyElementPastViewportEdge: overflowing.length > 0,
      overflowingElements: overflowing,
      subMinimumTapTargets: tiny.slice(0, 10),
      smallButUnreachableIgnored: hiddenSmall,
    }
  })
  console.log(JSON.stringify({ ok: true, path: target, viewport: `${width}x${height}`, scheme, screenshot: out, problems, ...facts }))
} catch (e) {
  console.log(JSON.stringify({ ok: false, path: target, viewport: `${width}x${height}`, scheme, error: String(e).slice(0, 300), problems }))
} finally {
  await browser.close()
}
