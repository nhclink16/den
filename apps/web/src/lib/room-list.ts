// What the room list shows for each room: its icon and a one-line preview.
// Kept free of the store so the rules can be pinned by tests.
import type { Channel, LastMessage, Message, RoomIcons } from './types'
import { snippet } from './klipy.ts'

/// Where a Lucide icon's file is served (see vite.config.ts).
export const iconUrl = (name: string) => `/lucide/${name}.svg`

// A room with no icon of its own gets one from its name, so a fresh Den doesn't
// open on a column of identical bubbles. First match wins.
const GUESSES: [RegExp, string][] = [
  [/minecraft|craft|build/, 'pickaxe'],
  [/game|gaming|play/, 'gamepad-2'],
  [/music|song|playlist|jam/, 'music'],
  [/clip|video|movie|film|watch/, 'clapperboard'],
  [/photo|pic|art|meme/, 'image'],
  [/plan|event|trip|calendar/, 'calendar'],
  [/food|cook|eat|recipe/, 'utensils'],
  [/code|dev|tech|bot|agent/, 'code'],
  [/book|read/, 'book-open'],
  [/news|link/, 'newspaper'],
  [/help|support|question/, 'life-buoy'],
  [/announce/, 'megaphone'],
]
export function defaultIcon(c: Pick<Channel, 'name' | 'kind'>) {
  if (c.kind === 'voice') return 'headphones'
  const name = c.name.toLowerCase()
  return GUESSES.find(([re]) => re.test(name))?.[1] ?? 'message-circle'
}
export const roomIcon = (c: Pick<Channel, 'name' | 'kind' | 'icon'>) => c.icon || defaultIcon(c)
/// A room's name as text, with the # only for people who chose it.
export const roomName = (c: Pick<Channel, 'name' | 'kind'>, mode: RoomIcons | undefined) => (mode === 'hash' && c.kind === 'text' ? `#${c.name}` : c.name)

/// The list's copy of a message that just arrived or changed.
export function lastOf(m: Message): LastMessage {
  return {
    id: m.id, author_id: m.author_id, created_at: m.created_at,
    content: [...m.content].slice(0, 200).join(''),
    attachment_types: m.attachments.map((a) => a.content_type),
    object_kinds: (m.objects ?? []).map((o) => o.kind),
  }
}

/// One line of text for a message: its words, or what it carried.
export function previewText(m: Pick<LastMessage, 'content' | 'attachment_types' | 'object_kinds'>) {
  const text = snippet(m.content.replace(/```[a-z0-9_-]*\n?/gi, '').replace(/\s+/g, ' ').trim())
  if (text) return text
  const types = m.attachment_types
  if (types.length) {
    const kind = types.every((t) => t.startsWith('image/')) ? 'Photo' : types.every((t) => t.startsWith('video/')) ? 'Video' : 'File'
    return types.length > 1 ? `${types.length} ${kind === 'Photo' ? 'photos' : kind === 'Video' ? 'videos' : 'files'}` : kind
  }
  const card = m.object_kinds[0]
  return card ? card[0]!.toUpperCase() + card.slice(1) : 'Message'
}

/// Lucide's names and search words, loaded when the picker first opens.
export type IconEntry = { name: string; words: string }
let catalog: Promise<IconEntry[]> | undefined
export function loadIcons(): Promise<IconEntry[]> {
  catalog ??= import('lucide-static/tags.json').then((m) =>
    Object.entries(m.default as Record<string, string[]>).map(([name, tags]) => ({ name, words: `${name.replace(/-/g, ' ')} ${tags.join(' ')}`.toLowerCase() })))
  catalog.catch(() => { catalog = undefined })
  return catalog
}

/// What the picker offers before anything is typed.
export const SUGGESTED = ['message-circle', 'flame', 'gamepad-2', 'pickaxe', 'music', 'headphones', 'clapperboard', 'popcorn', 'tv', 'image', 'camera', 'palette', 'calendar', 'party-popper', 'cake', 'gift', 'utensils', 'pizza', 'coffee', 'beer', 'code', 'bot', 'server', 'wrench', 'lightbulb', 'book-open', 'graduation-cap', 'briefcase', 'newspaper', 'megaphone', 'bell', 'life-buoy', 'heart', 'star', 'sparkles', 'trophy', 'dumbbell', 'bike', 'car', 'plane', 'tent', 'mountain', 'trees', 'sun', 'moon', 'dog', 'cat', 'ghost', 'skull', 'rocket', 'globe', 'map', 'house', 'dice-5', 'swords', 'face-grinning', 'shopping-cart', 'notebook-pen', 'lock']

/// Names first, then search words; every typed word has to match.
export function searchIcons(icons: IconEntry[], query: string, limit = 120): IconEntry[] {
  const words = query.toLowerCase().trim().split(/\s+/).filter(Boolean)
  if (!words.length) return SUGGESTED.map((name) => icons.find((i) => i.name === name)).filter((i): i is IconEntry => !!i)
  const hits = icons.filter((i) => words.every((w) => i.words.includes(w)))
  const byName = (i: IconEntry) => (i.name.startsWith(words.join('-')) ? 0 : words.every((w) => i.name.includes(w)) ? 1 : 2)
  return hits.sort((a, b) => byName(a) - byName(b) || a.name.localeCompare(b.name)).slice(0, limit)
}
