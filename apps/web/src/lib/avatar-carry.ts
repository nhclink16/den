import type { Appearance } from './types'

// Avatar shape and the online indicator used to stay on each device, in `den.layout`
// (store.svelte.ts's PREFS_KEY). This device's old choice goes up once, then goes.
const KEY = 'den.layout'
type Choices = Partial<Pick<Appearance, 'avatar_shape' | 'presence_style'>>
type Disk = Pick<Storage, 'getItem' | 'setItem'>
// [old key in den.layout, appearance field, its default, the other values]
const fields = [
  ['avatarShape', 'avatar_shape', 'theme', ['sharp', 'soft', 'round']],
  ['presence', 'presence_style', 'dot', ['ring', 'off']],
] as const
const read = (disk: Disk): Record<string, unknown> => { try { return JSON.parse(disk.getItem(KEY) || '{}') } catch { return {} } }

/** What to save from this device's old per-device choices, or null when there is
 *  nothing to do yet: no old copy here, or a server too old to keep them. A field
 *  goes up only while the server still has its default, so a choice already made
 *  on another device wins. An empty result means the old copy can just go. */
export function oldAvatarChoices(layout: object, a: Choices): Choices | null {
  const old = layout as Record<string, unknown>
  if (!fields.some(([key]) => key in old) || !fields.every(([, field]) => field in a)) return null
  const patch: Record<string, string> = {}
  for (const [key, field, fallback, others] of fields) {
    const mine = old[key]
    if (a[field] === fallback && (others as readonly unknown[]).includes(mine)) patch[field] = mine as string
  }
  return patch as Choices
}

let sending = false
/** Sends this device's old choices once. They stay on disk until the save works, so a
 *  reload halfway retries; while one is on its way, another sync or another server's
 *  Store in the desktop app sends nothing. */
export async function carryOldAvatarChoices(disk: Disk, a: Choices, save: (patch: Choices) => Promise<boolean>) {
  const patch = sending ? null : oldAvatarChoices(read(disk), a)
  if (!patch) return
  sending = true
  try {
    if (Object.keys(patch).length && !(await save(patch))) return
    const { avatarShape: _, presence: __, ...rest } = read(disk)
    disk.setItem(KEY, JSON.stringify(rest))
  } finally { sending = false }
}

/** A choice made now wins over this device's old one for that field. */
export function forgetOldAvatarChoice(disk: Disk, field: keyof Choices) {
  const layout = read(disk), key = fields.find(([, f]) => f === field)![0]
  if (!(key in layout)) return
  delete layout[key]
  disk.setItem(KEY, JSON.stringify(layout))
}
