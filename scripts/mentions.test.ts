import { test } from 'node:test'
import assert from 'node:assert/strict'
import { render, mentions } from '../apps/web/src/lib/markdown.ts'
import type { User } from '../apps/web/src/lib/types.ts'

const user = (id: string, username: string, display_name: string): User =>
  ({ id, username, display_name, bot: false, role: 'member' }) as User
const users = new Map<string, User>([
  ['u1', user('u1', 'Andy', 'Andy 🎧')],
  ['u2', user('u2', 'an.dy-2', 'The Other Andy')],
])

// These have to agree with mention_names() in crates/den-server/src/activity.rs.
// When they did not, the server linked the mention but the client drew plain text.
test('mentions match the server: any case, inner separators, trailing punctuation', () => {
  const linked = render('thanks @Andy. appreciate it', users)
  assert.match(linked, /<span class="mention" data-user="u1">@Andy 🎧<\/span>/)
  assert.match(linked, /<\/span>\. appreciate it/, 'the sentence keeps its period')
  assert.deepEqual(mentions('thanks @Andy. appreciate it', users), ['u1'])

  assert.match(render('yo @andy', users), /data-user="u1"/)
  assert.match(render('yo @an.dy-2 hi', users), /data-user="u2"/)
  assert.deepEqual(mentions('yo @an.dy-2 hi', users), ['u2'])
})

test('mentions stay out of addresses, code and unknown names', () => {
  assert.doesNotMatch(render('mail a@Andy now', users), /class="mention"/)
  assert.deepEqual(mentions('mail a@Andy now', users), [])
  assert.doesNotMatch(render('`@Andy`', users), /class="mention"/)
  assert.doesNotMatch(render('```\n@Andy\n```', users), /class="mention"/)
  assert.doesNotMatch(render('who is @nobody', users), /class="mention"/)
})
