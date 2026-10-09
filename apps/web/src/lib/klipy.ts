// GIF search. KLIPY's terms require searches and GIFs to load straight from
// this app, never through Den, and its links to be used exactly as given. So
// a sent GIF is just its link, and a GIF picture is saved as KLIPY's links.
import type { KlipyPicture } from './types'

const API = 'https://api.klipy.com/api/v1'

type Format = { url: string; width: number; height: number }
type Size = Partial<Record<'gif' | 'webp' | 'jpg' | 'mp4' | 'webm', Format>>
export type Gif = { id: number; slug: string; title: string; file: Partial<Record<'hd' | 'md' | 'sm' | 'xs', Size>>; blur_preview?: string }
export type Page = { gifs: Gif[]; more: boolean }

/** A message that is only a KLIPY GIF link is drawn as that GIF. */
export function klipyGif(content: string): string | null {
  const text = content.trim()
  return /^https:\/\/static[12]?\.klipy\.com\/\S+\.(?:gif|webp)$/.test(text) ? text : null
}
/** One-line previews (replies, notifications) say GIF instead of a long link. */
export const snippet = (content: string) => (klipyGif(content) ? 'GIF' : content)

const animated = (s: Size | undefined) => s?.webp ?? s?.gif
/** What the grid shows: small, so a page of results stays light. */
export const thumb = (g: Gif) => animated(g.file.sm) ?? animated(g.file.xs) ?? animated(g.file.md)
/** What a message carries. */
export const messageUrl = (g: Gif) => (animated(g.file.md) ?? animated(g.file.hd) ?? thumb(g))?.url
/** A profile picture is small; banners and wallpapers fill wide spaces. */
export function picture(g: Gif, size: 'md' | 'hd'): KlipyPicture | null {
  const s = g.file[size] ?? g.file.md ?? g.file.hd
  const moving = animated(s), still = s?.jpg
  return moving && still ? { url: moving.url, still_url: still.url, width: moving.width, height: moving.height } : null
}

// A stable, anonymous ID per person and server, so KLIPY can rank for them
// without learning who they are.
const ids = new Map<string, Promise<string>>()
function customer(origin: string, userId: string) {
  const key = `${origin}|${userId}`
  let id = ids.get(key)
  if (!id) {
    id = crypto.subtle.digest('SHA-256', new TextEncoder().encode(`den-klipy|${key}`))
      .then((hash) => [...new Uint8Array(hash)].slice(0, 16).map((b) => b.toString(16).padStart(2, '0')).join(''))
    ids.set(key, id)
  }
  return id
}
const locale = () => (navigator.language.split('-')[1] ?? 'us').toLowerCase()

export type Session = { key: string; origin: string; userId: string }

export async function search(s: Session, q: string, page: number, signal?: AbortSignal): Promise<Page> {
  const params = new URLSearchParams({ page: String(page), per_page: '24', customer_id: await customer(s.origin, s.userId), locale: locale() })
  if (q) params.set('q', q)
  const r = await fetch(`${API}/${encodeURIComponent(s.key)}/gifs/${q ? 'search' : 'trending'}?${params}`, { signal })
  if (r.status === 429) throw Error('GIF search is busy. Try again in a few minutes.')
  if (!r.ok) throw Error("GIF search isn't answering right now.")
  const body = await r.json() as { data?: { data?: Gif[]; has_next?: boolean } }
  return { gifs: (body.data?.data ?? []).filter((g) => thumb(g)), more: !!body.data?.has_next }
}

/** Tells KLIPY a GIF was used, which tunes what it shows this person next. */
export async function shared(s: Session, g: Gif, q: string) {
  const body = JSON.stringify({ customer_id: await customer(s.origin, s.userId), q })
  await fetch(`${API}/${encodeURIComponent(s.key)}/gifs/share/${encodeURIComponent(g.slug)}`, { method: 'POST', headers: { 'content-type': 'application/json' }, body }).catch(() => {})
}
