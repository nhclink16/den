// What the emoji picker shows: one flat list of entries, in display order. Each
// category is a run of entries that share `category`, so a later "Custom"
// category (DIRECTION.md: custom emoji) is a matter of adding entries to this
// list; the picker needs no other change to show and search them.
export type EmojiEntry = { emoji: string; name: string; keywords: string[]; category: string }

// The fields of emojibase-data's `en/data.json` this file reads.
export type RawEmoji = { label: string; hexcode: string; tags?: string[]; type: number; group?: number; order?: number; version: number }

// emojibase's groups, in its order. Group 2 holds the bare skin tone and hair
// components, which are not emoji anyone picks.
const CATEGORIES: Record<number, string> = {
  0: 'Smileys & emotion', 1: 'People & body', 3: 'Animals & nature', 4: 'Food & drink',
  5: 'Travel & places', 6: 'Activities', 7: 'Objects', 8: 'Symbols', 9: 'Flags',
}

// The exact form Unicode calls fully qualified. emojibase's own `emoji` field
// adds U+FE0F to anything with a text variant, so its thumbs up is not the 👍
// the quick reactions send, and the same reaction would count twice. The
// hexcode already carries FE0F inside sequences; a lone code point needs it
// only when its default look is text (❤ → ❤️).
export function fullyQualified(e: Pick<RawEmoji, 'hexcode' | 'type'>): string {
  const points = e.hexcode.split('-').map((h) => parseInt(h, 16))
  return String.fromCodePoint(...points) + (points.length === 1 && e.type === 0 ? '️' : '')
}

/// Picker entries from emojibase data, dropping anything newer than `newest`
/// (an Emoji version this device can draw).
export function toEntries(raw: RawEmoji[], newest = Infinity): EmojiEntry[] {
  return raw
    .filter((e) => e.group !== undefined && CATEGORIES[e.group] && e.version <= newest)
    .sort((a, b) => (a.order ?? 0) - (b.order ?? 0))
    .map((e) => ({ emoji: fullyQualified(e), name: e.label, keywords: e.tags ?? [], category: CATEGORIES[e.group!]! }))
}

/// Entries matching every word typed, best first. Per word: a whole name word,
/// then the start of a name word, then the start of a keyword, then anywhere
/// in the name. A whole-name match leads; ties go to the shorter (more
/// specific) name, then to the usual order. Colons are ignored, so ":fire:"
/// works too.
export function searchEmoji(entries: EmojiEntry[], query: string, limit = 160): EmojiEntry[] {
  const q = query.toLowerCase().replace(/:/g, ' ').trim().replace(/\s+/g, ' ')
  if (!q) return []
  const terms = q.split(' ')
  const scored: { e: EmojiEntry; score: number; i: number }[] = []
  entries.forEach((e, i) => {
    const name = e.name.toLowerCase()
    const words = name.split(/[\s:,_-]+/)
    let score = name === q ? -10 : 0
    for (const t of terms) {
      if (words.includes(t)) score += 0
      else if (words.some((w) => w.startsWith(t))) score += 1
      else if (e.keywords.some((k) => k.toLowerCase().startsWith(t))) score += 2
      else if (name.includes(t)) score += 3
      else return
    }
    scored.push({ e, score, i })
  })
  return scored
    .sort((a, b) => a.score - b.score || a.e.name.length - b.e.name.length || a.i - b.i)
    .slice(0, limit).map((s) => s.e)
}

/// The recent list after picking `emoji`: it moves to the front, once.
export function pushRecent(recent: string[], emoji: string, max = 24): string[] {
  return [emoji, ...recent.filter((e) => e !== emoji)].slice(0, max)
}

// Recent emoji live on this device, per server and per account.
const recentKey = (origin: string, userId: string) => `den.emoji.recent:${origin}:${userId}`
export function readRecent(origin: string, userId: string): string[] {
  try {
    const saved = JSON.parse(localStorage.getItem(recentKey(origin, userId)) || '[]')
    return Array.isArray(saved) ? saved.filter((e): e is string => typeof e === 'string') : []
  } catch { return [] }
}
export function saveRecent(origin: string, userId: string, recent: string[]) {
  try { localStorage.setItem(recentKey(origin, userId), JSON.stringify(recent)) } catch { /* Picking still works without storage. */ }
}

// An emoji the system font can't draw shows as an empty box (a single code
// point) or as its parts side by side (a sequence). Test one emoji per Emoji
// version, newest first, and keep everything up to the first one that draws.
// Emoji 12 (2019) is the floor: every system friends use draws it, and some
// fonts here draw older emoji in black and white, which this colour test would
// take for missing.
function draws(emoji: string, ctx: CanvasRenderingContext2D, ref: number): boolean {
  ctx.clearRect(0, 0, 40, 40)
  ctx.fillText(emoji, 0, 0)
  if (ctx.measureText(emoji).width > ref * 1.5) return false
  const px = ctx.getImageData(0, 0, 40, 40).data
  for (let i = 0; i < px.length; i += 4) if (px[i + 3]! > 0 && (px[i] !== px[i + 1] || px[i + 1] !== px[i + 2])) return true
  return false
}
function newestDrawn(raw: RawEmoji[]): number {
  const ctx = document.createElement('canvas').getContext('2d', { willReadFrequently: true })
  if (!ctx) return Infinity
  ctx.canvas.width = ctx.canvas.height = 40
  ctx.font = '32px sans-serif'
  ctx.textBaseline = 'top'
  const ref = ctx.measureText('😀').width
  const versions = [...new Set(raw.map((e) => e.version))].filter((v) => v > 12).sort((a, b) => b - a)
  for (const v of versions) {
    const of = raw.filter((e) => e.version === v && e.group !== undefined && CATEGORIES[e.group])
    // A lone code point tests the font most directly; some versions only added sequences.
    const sample = of.find((e) => !e.hexcode.includes('-')) ?? of[0]
    if (sample && draws(fullyQualified(sample), ctx, ref)) return v
  }
  return 12
}

// Loaded the first time any picker opens, then shared.
let loading: Promise<EmojiEntry[]> | undefined
export function loadEmoji(): Promise<EmojiEntry[]> {
  loading ??= import('emojibase-data/en/data.json').then((m) => {
    const raw = m.default as unknown as RawEmoji[]
    return toEntries(raw, newestDrawn(raw))
  }, (err) => { loading = undefined; throw err })
  return loading
}
