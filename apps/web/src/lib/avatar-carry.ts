import type { Appearance } from './types'

type Choices = Partial<Pick<Appearance, 'avatar_shape' | 'presence_style'>>
// [old key in den.layout, appearance field, its default, the other values]
const fields = [
  ['avatarShape', 'avatar_shape', 'theme', ['sharp', 'soft', 'round']],
  ['presence', 'presence_style', 'dot', ['ring', 'off']],
] as const

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
