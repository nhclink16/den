import { test } from 'node:test'
import assert from 'node:assert/strict'
import { shownActivities, activitySince, activityVerb } from '../apps/web/src/lib/activity.ts'
import type { Activity } from '../apps/web/src/lib/types.ts'

const at = (slot: string, kind: Activity['kind'], name: string, started_at: number, extra: Partial<Activity> = {}): Activity =>
  ({ slot, kind, name, details: null, image_url: null, started_at, expires_at: started_at + 90_000, ...extra })

test('the most telling activity leads, and one game reported twice shows once', () => {
  const shown = shownActivities([
    at('desktop', 'using', 'Firefox', 5),
    at('spotify', 'listening', 'Midnight City', 4, { details: 'M83' }),
    at('desktop', 'playing', 'Minecraft', 3),
    at('minecraft', 'playing', 'minecraft', 2, { details: 'on the den' }),
  ])
  assert.deepEqual(shown.map((a) => [a.kind, a.name]), [['playing', 'minecraft'], ['listening', 'Midnight City'], ['using', 'Firefox']])
  assert.equal(shown[0]!.details, 'on the den', 'the copy that says more wins')
  assert.deepEqual(shownActivities(undefined), [])
})

test('elapsed time reads the way people say it', () => {
  const now = 10 * 3_600_000
  assert.equal(activitySince(now - 20_000, now), 'just started')
  assert.equal(activitySince(now - 12 * 60_000, now), 'for 12m')
  assert.equal(activitySince(now - 2 * 3_600_000, now), 'for 2h')
  assert.equal(activitySince(now - (2 * 60 + 5) * 60_000, now), 'for 2h 5m')
  assert.equal(activityVerb('listening'), 'Listening to')
})
