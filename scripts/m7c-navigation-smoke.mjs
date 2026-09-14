import { SmokeCleanup } from './smoke-cleanup.mjs'
// A send may finish after expanding the call has removed its composer.
import { chromium } from 'playwright-core'
import { readFile } from 'node:fs/promises'
import assert from 'node:assert/strict'
const base = process.env.DEN_SMOKE_URL || 'http://localhost:7001'
const creds = JSON.parse(await readFile(process.env.DEN_SMOKE_CREDENTIALS || `${process.env.HOME}/.local/share/den-dev/credentials.json`, 'utf8'))
const cleanup = await SmokeCleanup.login(base, 'nicholas', creds.users?.nicholas || creds.password)
const browser = await chromium.launch({ executablePath: '/usr/bin/chromium', headless: true, args: ['--no-sandbox'] })
try {
  const page = await browser.newPage(), errors = []
  cleanup.watch(page.context())
  page.on('pageerror', e => errors.push(e.message))
  await page.goto(base)
  await page.getByLabel('Username', { exact: true }).fill('nicholas')
  await page.getByLabel('Password', { exact: true }).fill(creds.users?.nicholas || creds.password)
  await page.getByRole('button', { name: 'Come in', exact: true }).click()
  await page.locator('nav.side a.row').filter({ hasText: 'general' }).click()
  let release, responseReady
  const response = new Promise(r => { responseReady = r }), gate = new Promise(r => { release = r })
  await page.route('**/channels/*/messages', async route => {
    if (route.request().method() !== 'POST') return route.continue()
    const result = await route.fetch(); cleanup.record(new URL(route.request().url()).pathname, await result.json()); responseReady(); await gate; await route.fulfill({ response: result })
  })
  await page.locator('textarea').fill('M7c regression: leave the composer while a send is pending.')
  await page.locator('textarea').press('Enter'); await response
  await page.locator('a[title="Settings"]').click()
  await page.locator('textarea').waitFor({ state: 'detached' })
  release()
  await page.waitForTimeout(300)
  await page.unrouteAll()
  const general = await page.evaluate(async () => (await (await fetch('/channels')).json()).find(c => c.name === 'general'))
  const sample = await page.evaluate(async id => (await (await fetch(`/channels/${id}/messages`)).json())[0], general.id)
  let olderReady, releaseOlder
  const older = new Promise(r => { olderReady = r }), olderGate = new Promise(r => { releaseOlder = r })
  // A full first page is the server's signal that more history may exist.
  const history = Array.from({ length: 50 }, (_, i) => ({ ...sample, id: `history-${String(i).padStart(3, '0')}`, content: 'History pagination fixture', objects: [], upload_ids: [], reply_to: null }))
  await page.route(`**/channels/${general.id}/messages?*`, async route => {
    if (new URL(route.request().url()).searchParams.has('before')) {
      olderReady(); await olderGate; await route.fulfill({ json: [] })
    } else await route.fulfill({ json: history })
  })
  await page.goto(`${base}/c/${general.id}`)
  await page.getByText('History pagination fixture', { exact: true }).first().waitFor()
  await page.locator('.list').evaluate(el => { el.scrollTop = 0; el.dispatchEvent(new Event('scroll')) })
  await older
  await page.locator('a[title="Settings"]').click()
  await page.locator('.list').waitFor({ state: 'detached' })
  releaseOlder(); await page.waitForTimeout(300)
  assert.deepEqual(errors, [], 'late send and history responses must not touch removed elements')
  console.log('M7c navigation regression passed: late send and history responses after view teardown.')
} finally { await cleanup.finish(browser) }
