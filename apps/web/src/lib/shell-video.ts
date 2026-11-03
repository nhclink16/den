// Den Shell (the iPhone app) runs calls natively, and so draws their video natively too: the page can't show the native
// engine's video. The video goes *behind* the page, and the page leaves a see-through hole where each video tile is.
// So whatever the page draws on a tile or over it (the name, the talking ring, a sheet and its dimming, a menu, a toast,
// another screen) is on top of the video by itself, and a native view that is a frame late can't show anywhere else:
// everywhere but the holes the page stays opaque.
//
// While any slot is up, <html data-native-video> moves the page's background from <body> to a fixed layer masked with
// the holes (app.css), and each wallpaper layer gets the same holes in its own coordinates. A tile turns see-through
// (`data-native="on"`) only once the shell says its video is drawn where the page last said. Anything that moves a tile
// (a scroll, a layout change, people joining or leaving, a rotation, the keyboard) closes its hole in that same frame,
// so it shows its face again; once the tile has stayed put for a frame the page tells the shell, and the hole opens when
// the shell has the video there. The shell only gets slots that are on screen, and subscribes to just those videos.
// Everywhere else `window.denShell.video` is missing and none of this runs.

type Rect = { x: number; y: number; width: number; height: number }
export type VideoSpec = { identity: string; source: 'camera' | 'screen'; name?: string; fit: 'cover' | 'contain' }
/** `rect`: the whole tile in the web view's points; `visible`: the part of it on screen. `fill` shows round a share. */
export type ShellVideoSlot = VideoSpec & { key: string; rect: Rect; visible: Rect; radius: number; fill: string }
/** Every slot each time, never a change: a missed one is fixed by the next. `seq` grows with each. */
export type ShellVideoCommand = { type: 'slots'; seq: number; slots: ShellVideoSlot[] }
/** The slots of placement `seq` whose video is drawn there. `rev` grows with each reply and push: they travel apart. */
export type ShellVideoReply = { seq: number; rev: number; ready: string[] }
/** `ready`: that set changed (a first frame came, a video went). `place`: the shell moved things itself; say again. */
export type ShellVideoEvent = { type: 'ready'; seq: number; rev: number; ready: string[] } | { type: 'place' }

export const videoKey = (s: VideoSpec) => s.source === 'camera' ? `${s.identity}|camera` : `${s.identity}|screen|${s.name}`

type Measure = { rect: Rect; visible: Rect }
type Slot = { spec: VideoSpec; key: string; m: Measure | null; seen: string; fill: string; open: boolean }
const slots = new Map<HTMLElement, Slot>()
/** The last placement sent, and what the shell has drawn as of it (by key: where, and whether its video is in). */
/** `sent` when the shell must hear again whatever is there, even nothing. */
const AGAIN = '\u0000'
let seq = 0, rev = 0, sent = '', placed = new Map<string, string>(), ready = new Set<string>()
let pending: { seq: number; at: Map<string, string> } | null = null
let frame = 0, timer: ReturnType<typeof setInterval> | undefined, watch: MutationObserver | undefined, theme: MutationObserver | undefined
let sized: ResizeObserver | undefined

const round = (n: number) => Math.round(n * 100) / 100
const sig = (m: Measure | null) => m ? [m.rect.x, m.rect.y, m.rect.width, m.rect.height, m.visible.x, m.visible.y, m.visible.width, m.visible.height].map(round).join() : ''
function intersect(a: Rect, b: DOMRect | Rect): Rect {
  const x = Math.max(a.x, b.x), y = Math.max(a.y, b.y)
  return { x, y, width: Math.max(0, Math.min(a.x + a.width, b.x + b.width) - x), height: Math.max(0, Math.min(a.y + a.height, b.y + b.height) - y) }
}

/** The tile, and the part of it its scrolling ancestors and the screen leave showing. */
function measure(node: HTMLElement): Measure | null {
  if (!node.isConnected) return null
  const r = node.getBoundingClientRect()
  const rect = { x: r.x, y: r.y, width: r.width, height: r.height }
  let visible = intersect(rect, { x: 0, y: 0, width: innerWidth, height: innerHeight })
  for (let el = node.parentElement; el && el !== document.body; el = el.parentElement) {
    const s = getComputedStyle(el)
    if (s.overflowX !== 'visible' || s.overflowY !== 'visible') visible = intersect(visible, el.getBoundingClientRect())
  }
  return visible.width > 0 && visible.height > 0 ? { rect, visible } : null
}

/** Where to look, and whether the page is pinch-zoomed (its pixels aren't the screen's points then). */
const zoomed = () => { const z = window.visualViewport; return !!z && Math.abs(z.scale - 1) > 0.01 }

/** Measures every slot. One that moved closes now, in this frame; once nothing has moved for a frame the shell hears
 *  where they all are. */
function tick() {
  cancelAnimationFrame(frame); frame = 0
  const zoom = zoomed()
  let moved = false
  for (const [node, slot] of slots) {
    // Its corners too: a theme with other corners needs the shell's view rounded again.
    const m = zoom ? null : measure(node), now = m ? `${sig(m)}|${getComputedStyle(node).borderTopLeftRadius}` : ''
    if (now === slot.seen) continue
    slot.seen = now; slot.m = m; moved = true
  }
  if (moved) { refresh(); frame = requestAnimationFrame(tick); return }
  // A wallpaper that came or went, or a host that moved, needs its holes again.
  cut()
  const shown = [...slots].filter(([, s]) => s.m)
  const want = shown.map(([, s]) => `${s.key}@${s.seen}`).sort().join(';')
  if (want === sent) return
  sent = want
  const at = new Map(shown.map(([, s]) => [s.key, s.seen]))
  // Until the shell answers this one, a tile counts as placed only where it was and still is: one that moved away and
  // back must wait, the shell's view is on its way somewhere else.
  for (const [key, where] of placed) if (at.get(key) !== where) placed.delete(key)
  pending = { seq: ++seq, at }
  const command: ShellVideoCommand = { type: 'slots', seq, slots: shown.map(([node, s]) => {
    if (!s.open) s.fill = getComputedStyle(node).backgroundColor
    return { ...s.spec, key: s.key, rect: s.m!.rect, visible: s.m!.visible, radius: parseFloat(getComputedStyle(node).borderTopLeftRadius) || 0, fill: s.fill }
  }) }
  void window.denShell!.video!(command).then((r) => accept(r.seq, r.rev, r.ready), () => {})
}

/** The shell's word on a placement: only on the newest one, and only its newest word. */
function accept(at: number, said: number, keys: string[]) {
  if (!pending || at !== pending.seq || said <= rev) return
  rev = said
  placed = pending.at; ready = new Set(keys)
  refresh()
}

/** Opens the holes whose video is drawn where the tile is now, closes the rest, and cuts the page's layers to match. */
function refresh() {
  let changed = false
  for (const [node, slot] of slots) {
    const open = !!slot.m && placed.get(slot.key) === slot.seen && ready.has(slot.key)
    if (open === slot.open) continue
    slot.open = open; changed = true
    node.dataset.native = open ? 'on' : 'off'
  }
  if (changed) cut()
}

/** Only a real change: setting a style is a mutation, and mutations call `tick`. */
function set(el: HTMLElement, name: string, value: string) { if (el.style.getPropertyValue(name) !== value) el.style.setProperty(name, value) }

/** The holes as a mask image for a layer whose box is `box` on screen (scaled by `sx`/`sy` about its centre, as the
 *  wallpaper is): opaque everywhere but the open tiles. */
export function holes(box: Rect, open: { rect: Rect; visible: Rect; radius: number }[], sx = 1, sy = 1, tx = 0, ty = 0) {
  if (!open.length) return 'none'
  const cx = box.width / 2, cy = box.height / 2
  const lx = (x: number) => round(cx + (x - box.x - cx - tx) / sx), ly = (y: number) => round(cy + (y - box.y - cy - ty) / sy)
  const local = (r: Rect) => ({ x: lx(r.x), y: ly(r.y), width: round(r.width / sx), height: round(r.height / sy) })
  let clips = '', cuts = ''
  open.forEach((h, i) => {
    const r = local(h.rect), v = local(h.visible)
    clips += `<clipPath id="c${i}"><rect x="${v.x}" y="${v.y}" width="${v.width}" height="${v.height}"/></clipPath>`
    cuts += `<rect x="${r.x}" y="${r.y}" width="${r.width}" height="${r.height}" rx="${round(h.radius / sx)}" ry="${round(h.radius / sy)}" clip-path="url(#c${i})"/>`
  })
  const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="${round(box.width)}" height="${round(box.height)}"><defs>${clips}</defs>`
    + `<mask id="m"><rect width="100%" height="100%" fill="#fff"/>${cuts}</mask><rect width="100%" height="100%" mask="url(#m)"/></svg>`
  return `url("data:image/svg+xml,${encodeURIComponent(svg)}")`
}

/** Puts the open holes in the page's background layer and in every wallpaper layer, each in its own coordinates. */
function cut() {
  const open = [...slots].flatMap(([node, s]) => s.open && s.m ? [{ ...s.m, radius: parseFloat(getComputedStyle(node).borderTopLeftRadius) || 0 }] : [])
  // On <body>, whose ::before uses it: <html>'s style changing means a new theme (see `start`).
  set(document.body, '--den-holes', holes({ x: 0, y: 0, width: innerWidth, height: innerHeight }, open))
  for (const host of document.querySelectorAll<HTMLElement>('.bg-host')) {
    const r = host.getBoundingClientRect(), b = getComputedStyle(host)
    const box = { x: r.x + parseFloat(b.borderLeftWidth), y: r.y + parseFloat(b.borderTopWidth), width: host.clientWidth, height: host.clientHeight }
    for (const part of ['before', 'after'] as const) {
      const s = getComputedStyle(host, `::${part}`)
      // matrix(a, b, c, d, e, f): the wallpaper is only ever scaled (and maybe moved), about its centre.
      const [a = 1, , , d = 1, e = 0, f = 0] = s.transform.startsWith('matrix(') ? s.transform.slice(7, -1).split(',').map(Number) : []
      set(host, `--den-holes-${part}`, s.content === 'none' ? 'none' : holes(box, open, a, d, e, f))
    }
  }
}

function kick() { if (slots.size && !frame) frame = requestAnimationFrame(tick) }
/** A scroll moves the tiles on the compositor before the page hears of it: close at once, place again when it stops. */
function scrolled() { for (const slot of slots.values()) slot.seen = 'scrolling'; refresh(); kick() }
/** A new theme or wallpaper: shut every hole, so the tiles show their own colour and corners again, and place them anew
 *  with those (a share's letterbox is the tile's colour). */
const rethemed = () => { sent = AGAIN; placed = new Map(); pending = null; scrolled() }
function place() { sent = AGAIN; kick() }
function visible() { if (document.visibilityState === 'visible') place() }

function start() {
  document.documentElement.dataset.nativeVideo = ''
  // Anything in the page can move a tile: a new person, a layout, the faces under the tiles, a font.
  watch = new MutationObserver(tick)
  watch.observe(document.body, { subtree: true, childList: true, attributes: true, attributeFilter: ['style', 'class'] })
  // The theme and the wallpaper are set on <html>.
  theme = new MutationObserver(rethemed)
  theme.observe(document.documentElement, { attributes: true, attributeFilter: ['style', 'data-bg-scope', 'data-sidebar-bg', 'data-theme'] })
  sized = new ResizeObserver(kick)
  addEventListener('scroll', scrolled, { capture: true, passive: true })
  addEventListener('resize', kick)
  window.visualViewport?.addEventListener('resize', kick)
  document.addEventListener('visibilitychange', visible)
  // Anything else that moves one, caught late rather than never.
  timer = setInterval(tick, 500)
  window.denShell!.onVideo = (e) => { if (e.type === 'place') place(); else accept(e.seq, e.rev, e.ready) }
}

function stop() {
  cancelAnimationFrame(frame); frame = 0
  watch?.disconnect(); theme?.disconnect(); sized?.disconnect(); clearInterval(timer)
  removeEventListener('scroll', scrolled, { capture: true })
  removeEventListener('resize', kick)
  window.visualViewport?.removeEventListener('resize', kick)
  document.removeEventListener('visibilitychange', visible)
  delete document.documentElement.dataset.nativeVideo
  document.body.style.removeProperty('--den-holes')
  for (const host of document.querySelectorAll<HTMLElement>('.bg-host')) { host.style.removeProperty('--den-holes-before'); host.style.removeProperty('--den-holes-after') }
  sent = ''; placed = new Map(); ready = new Set(); pending = { seq: ++seq, at: new Map() }
  void window.denShell!.video!({ type: 'slots', seq, slots: [] }).catch(() => {})
}

/** A video tile's slot (`spec` null: none). The tile keeps its face until the shell's video is in. */
export function nativeVideo(node: HTMLElement, spec: VideoSpec | null) {
  function add(next: VideoSpec) {
    if (!slots.size) start()
    slots.set(node, { spec: next, key: videoKey(next), m: null, seen: '', fill: '', open: false })
    node.dataset.native = 'off'
    sized?.observe(node)
    tick()
  }
  function remove() {
    if (!slots.delete(node)) return
    sized?.unobserve(node)
    delete node.dataset.native
    // Its hole closes with it, in this frame: the shell's view may stay behind the page a frame longer.
    if (slots.size) { cut(); tick() } else stop()
  }
  if (spec) add(spec)
  return {
    update(next: VideoSpec | null) {
      const old = slots.get(node)
      if (old && next && videoKey(next) === old.key && next.fit === old.spec.fit) return
      remove()
      if (next) add(next)
    },
    destroy: remove,
  }
}
