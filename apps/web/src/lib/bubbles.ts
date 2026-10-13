// How a conversation is drawn as bubbles: where one person's run of messages
// starts and ends, where the time goes, and what colour each bubble is. Pure,
// so the Node tests run it.
import type { BubbleColors, ThemeColors } from './types'
import { contrast, fromLab, lab, readable } from './theme-color.ts'
import { sameDay } from './time.ts'

/** A pause this long, or a new day, puts the time between two messages. */
export const TIME_BREAK = 15 * 60_000

type Placed = { id: string; author_id: string; created_at: string }
/** `head` starts a run (the name goes above it), `tail` ends one (the picture
 *  goes beside it), `time` puts the time above it. */
export type Place = { head: boolean; tail: boolean; time: boolean }

/** One person's messages in a row stack into one run, until the time shows
 *  again or the unread line cuts in. */
export function places(list: Placed[], firstUnread?: string): Place[] {
  const out: Place[] = []
  list.forEach((m, i) => {
    const prev = list[i - 1]
    const time = !prev || !sameDay(prev.created_at, m.created_at) || Date.parse(m.created_at) - Date.parse(prev.created_at) >= TIME_BREAK
    const head = time || prev!.author_id !== m.author_id || m.id === firstUnread
    if (prev && head) out[i - 1]!.tail = true
    out.push({ head, tail: false, time })
  })
  if (out.length) out[out.length - 1]!.tail = true
  return out
}

/** A person's own hue: their chosen profile colour, else a stable hash of their id. */
export function personHue(userId: string, chosen?: string | null): number {
  if (chosen) {
    const [, a, b] = lab(chosen) as [number, number, number]
    return Math.round((Math.atan2(b, a) * 180 / Math.PI + 360) % 360)
  }
  return ([...userId].reduce((h, c) => Math.imul(h ^ c.charCodeAt(0), 16777619), 2166136261) >>> 0) % 360
}

/** The colour a person's bubbles are tinted with: the one they chose in Profile,
 *  else their picture's fallback hue as a mid colour. */
export function personColor(userId: string, chosen?: string | null): string {
  if (chosen && /^#[0-9a-f]{6}$/i.test(chosen)) return chosen.toLowerCase()
  const h = personHue(userId) * Math.PI / 180
  return fromLab(.66, .12 * Math.cos(h), .12 * Math.sin(h))
}

/** The ink wash behind code and mentions inside a bubble (MessageItem uses 9%). */
const WASH = .09
/** `color-mix(in srgb, …)`, and what a translucent layer over a colour comes to. */
function srgbMix(from: string, to: string, amount: number) {
  const rgb = (hex: string) => hex.slice(1).match(/../g)!.map((v) => parseInt(v, 16))
  const a = rgb(from), b = rgb(to)
  return '#' + a.map((v, i) => Math.round(v + (b[i]! - v) * amount).toString(16).padStart(2, '0')).join('')
}
function mix(from: string, to: string, amount: number) {
  const a = lab(from), b = lab(to)
  return fromLab(...(a.map((v, i) => v + (b[i]! - v) * amount) as [number, number, number]))
}

/** The raised surface tinted with `color`, as strongly as the theme allows while
 *  text and secondary text on it keep 4.5:1, text also on the faint ink wash
 *  behind code and mentions. A palette whose own raised surface reads below
 *  that (or Contrast turned down) only promises no worse than its plain bubble.
 *  The plain surface when nothing does. */
export function tint(color: string, c: Pick<ThemeColors, 'bg' | 'bg3' | 'ink' | 'ink2'>): string {
  const pale = lab(c.bg)[0]! > .5
  // Text sits on the bubble and on the wash; secondary text ("edited") only on the bubble.
  const surfaces = (fill: string) => ({ ink: [fill, srgbMix(fill, c.ink, WASH)], ink2: [fill] })
  const plain = surfaces(c.bg3)
  const floor = (role: 'ink' | 'ink2') => Math.min(4.5, ...plain[role].map((bg) => contrast(c[role], bg)))
  const need = { ink: floor('ink'), ink2: floor('ink2') }
  const reads = (fill: string) => (['ink', 'ink2'] as const).every((role) => surfaces(fill)[role].every((bg) => contrast(c[role], bg) >= need[role]))
  for (let amount = pale ? .3 : .4; amount > .01; amount -= .02) {
    const fill = mix(c.bg3, color, amount)
    if (reads(fill)) return fill
  }
  return c.bg3
}

/** Secondary text as Contrast leaves it: app.css mixes it toward the text (above
 *  100) or the background (below), the same way theme-runtime does. */
export function withContrast<C extends Pick<ThemeColors, 'bg' | 'ink' | 'ink2'>>(c: C, percent = 100): C {
  const amount = Math.max(80, Math.min(120, percent))
  if (amount === 100) return c
  const d = Math.abs(amount - 100) * .55 / 100, toward = amount > 100 ? c.ink : c.bg
  return { ...c, ink2: srgbMix(c.ink2, toward, d) }
}

const cache = new Map<string, string>()
/** One bubble's fill, or undefined for the plain grey one. `mentionsMe` wins:
 *  a message that names you is lit in the accent whatever the mode. */
export function bubbleFill(mode: BubbleColors | undefined, mine: boolean, mentionsMe: boolean, person: string, accent: string, c: ThemeColors): string | undefined {
  const color = mentionsMe || (mine && mode === 'plain') ? accent : mode === 'plain' || (mode === 'mine' && !mine) ? undefined : person
  if (!color) return undefined
  const key = `${color}${c.bg}${c.bg3}${c.ink}${c.ink2}`
  let fill = cache.get(key)
  if (!fill) {
    // A live theme edit makes a new palette on every drag; don't keep them all.
    if (cache.size > 500) cache.clear()
    cache.set(key, (fill = tint(color, c)))
  }
  return fill
}

/** A name's colour in the plain feed, where there are no bubbles to colour:
 *  the person's colour, moved until it reads on the page. Undefined for plain text. */
export function nameInk(mode: BubbleColors | undefined, mine: boolean, person: string, c: ThemeColors): string | undefined {
  if (mode === 'plain' || (mode === 'mine' && !mine)) return undefined
  const key = `name${person}${c.bg}${c.bg2}`
  let ink = cache.get(key)
  if (!ink) {
    if (cache.size > 500) cache.clear()
    cache.set(key, (ink = readable(person, [c.bg, c.bg2])))
  }
  return ink
}
