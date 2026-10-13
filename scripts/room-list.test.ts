import { test } from 'node:test'
import assert from 'node:assert/strict'
import { existsSync, readFileSync } from 'node:fs'
import { SUGGESTED, defaultIcon, lastOf, previewText, roomName, searchIcons, type IconEntry } from '../apps/web/src/lib/room-list.ts'
import type { Message } from '../apps/web/src/lib/types.ts'

const lucide = new URL('../apps/web/node_modules/lucide-static/', import.meta.url)
const tags = JSON.parse(readFileSync(new URL('tags.json', lucide), 'utf8')) as Record<string, string[]>
const icons: IconEntry[] = Object.entries(tags).map(([name, t]) => ({ name, words: `${name.replace(/-/g, ' ')} ${t.join(' ')}`.toLowerCase() }))
const preview = (content: string, attachment_types: string[] = [], object_kinds: string[] = []) => previewText({ content, attachment_types, object_kinds })

test('every icon Den names itself ships with Lucide, with a file to draw', () => {
  const names = [...SUGGESTED, ...['general', 'minecraft', 'games', 'music', 'clips', 'photos', 'plans', 'food', 'code', 'books', 'news', 'help', 'announcements'].map((name) => defaultIcon({ name, kind: 'text' })), defaultIcon({ name: 'hangout', kind: 'voice' })]
  for (const name of names) {
    assert.ok(tags[name], `${name} is not a Lucide icon`)
    assert.ok(existsSync(new URL(`icons/${name}.svg`, lucide)), `${name}.svg is missing`)
  }
})

test('rooms without an icon get one from their name, and a plain bubble otherwise', () => {
  assert.equal(defaultIcon({ name: 'minecraft-chat', kind: 'text' }), 'pickaxe')
  assert.equal(defaultIcon({ name: 'Movie-Night', kind: 'text' }), 'clapperboard')
  assert.equal(defaultIcon({ name: 'general', kind: 'text' }), 'message-circle')
  assert.equal(defaultIcon({ name: 'music', kind: 'voice' }), 'headphones')
})

test('a # appears only for people who chose it, and never on voice rooms', () => {
  assert.equal(roomName({ name: 'general', kind: 'text' }, 'hash'), '#general')
  assert.equal(roomName({ name: 'general', kind: 'text' }, 'icons'), 'general')
  assert.equal(roomName({ name: 'general', kind: 'text' }, 'none'), 'general')
  assert.equal(roomName({ name: 'general', kind: 'text' }, undefined), 'general')
  assert.equal(roomName({ name: 'hangout', kind: 'voice' }, 'hash'), 'hangout')
})

test('a preview is one line of words, or says what was sent', () => {
  assert.equal(preview('bring snacks\n\nfor   the crew'), 'bring snacks for the crew')
  assert.equal(preview('```js\nlet x = 1\n```'), 'let x = 1')
  assert.equal(preview('https://static.klipy.com/ii/abc/1.gif'), 'GIF')
  assert.equal(preview('', ['image/png']), 'Photo')
  assert.equal(preview('', ['image/png', 'image/jpeg']), '2 photos')
  assert.equal(preview('', ['video/mp4']), 'Video')
  assert.equal(preview('', ['image/png', 'application/pdf']), '2 files')
  assert.equal(preview('look', ['image/png']), 'look')
  assert.equal(preview('', [], ['canvas']), 'Canvas')
  assert.equal(preview(''), 'Message')
})

test("a live message becomes the same preview the server sends, cut by characters", () => {
  const m = { id: '01B', channel_id: 'c', author_id: 'u', content: '😀'.repeat(250), created_at: 'now', edited_at: null, reply_to: null,
    attachments: [{ content_type: 'image/png' }], objects: [{ kind: 'canvas' }] } as unknown as Message
  const last = lastOf(m)
  assert.equal([...last.content].length, 200)
  assert.deepEqual([last.attachment_types, last.object_kinds], [['image/png'], ['canvas']])
})

test('icon search puts names before tags and needs every word', () => {
  assert.equal(searchIcons(icons, '')[0]!.name, SUGGESTED[0])
  assert.equal(searchIcons(icons, 'flame')[0]!.name, 'flame')
  assert.ok(searchIcons(icons, 'fire').some((i) => i.name === 'flame'), 'tags count')
  const both = searchIcons(icons, 'game controller')
  assert.ok(both.length && both.every((i) => i.words.includes('game') && i.words.includes('controller')))
  assert.equal(searchIcons(icons, 'zzzz-nothing').length, 0)
  assert.ok(searchIcons(icons, 'a').length <= 120)
})
