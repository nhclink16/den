import { test } from 'node:test'
import assert from 'node:assert/strict'
import { readdirSync, readFileSync } from 'node:fs'
import { ENTRIES, findSettings, visibleSections } from '../apps/web/src/lib/settings-search.ts'

const ui = new URL('../apps/web/src/ui/', import.meta.url)
const sources = readdirSync(ui).filter((f) => f.endsWith('.svelte')).map((f) => readFileSync(new URL(f, ui), 'utf8')).join('\n')

test('a member never sees server sections or admin-only rows, an admin does', () => {
  const member = visibleSections(false, false), admin = visibleSections(true, false)
  for (const id of ['members', 'invites', 'rooms', 'plugins']) {
    assert.ok(!member.some((s) => s.id === id), `${id} hidden from members`)
    assert.ok(admin.some((s) => s.id === id), `${id} shown to admins`)
  }
  for (const q of ['invite', 'server name', 'canvas', 'rename', 'server default']) {
    assert.deepEqual(findSettings(q, member, false), [], `"${q}" finds nothing for a member`)
    assert.ok(findSettings(q, admin, true).length, `"${q}" finds something for an admin`)
  }
})

test('the desktop app has no "Desktop app" download page', () => {
  assert.ok(visibleSections(false, false).some((s) => s.id === 'desktop'))
  assert.ok(!visibleSections(false, true).some((s) => s.id === 'desktop'))
  assert.deepEqual(findSettings('download', visibleSections(false, true), false), [])
})

test('the iPhone app hides computer pages and keeps developer ones behind Developer', () => {
  const phone = visibleSections(false, false, true), browser = visibleSections(false, false)
  for (const id of ['layout', 'machines', 'desktop']) assert.ok(!phone.some((s) => s.id === id), `${id} hidden in the iPhone app`)
  // Access, API keys and Agents still open (Developer lists them), and Developer is only in the iPhone app.
  for (const id of ['access', 'keys', 'agents', 'developer']) assert.ok(phone.some((s) => s.id === id), `${id} opens in the iPhone app`)
  assert.ok(!browser.some((s) => s.id === 'developer'))
  assert.ok(browser.some((s) => s.id === 'layout') && browser.some((s) => s.id === 'machines'))
  // Its calls don't use the browser's devices, so a search doesn't offer them.
  assert.ok(!findSettings('microphone', phone, false, true).some((h) => h.entry?.computer))
  assert.ok(findSettings('gain', phone, false, true).every((h) => !h.entry))
  assert.deepEqual(findSettings('shortcuts', phone, false, true), [])
  assert.ok(findSettings('microphone', browser, false).length)
  assert.equal(findSettings('push to talk', phone, false, true)[0]?.entry?.find, 'input-mode')
})

test('a section by name comes first, then rows whose name starts with the word', () => {
  const sections = visibleSections(true, false)
  const voice = findSettings('voice', sections, true)
  assert.equal(voice[0]!.section.id, 'voice')
  assert.equal(voice[0]!.entry, undefined)
  const dark = findSettings('dark', sections, true)
  assert.equal(dark[0]!.entry?.find, 'mode')
  assert.equal(findSettings('wallpaper blur', sections, true)[0]!.entry?.find, 'wallpaper')
  assert.deepEqual(findSettings('   ', sections, true), [])
  assert.deepEqual(findSettings('zzzz', sections, true), [])
})

test('every row a search can land on exists in the settings pages', () => {
  for (const e of ENTRIES) {
    const marked = new RegExp(`find=(?:"${e.find}"|\\{[^}]*'${e.find}'[^}]*\\})`)
    assert.match(sources, marked, `${e.section}: no row marked "${e.find}"`)
  }
})
