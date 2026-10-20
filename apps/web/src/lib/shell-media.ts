// Den Shell (the iPhone app) picks photos with iOS's own picker, in one step: the page's <input> shows WebKit's
// "Photo Library / Take Photo / Choose File" menu first. It also shows iOS's share sheet, which `navigator.share`
// didn't bring up there. Shells from before this, browsers and the desktop keep the <input> and `navigator.share`.

/** A picked photo or video, kept by the shell until the page has read it (one over 1 GB isn't kept). */
export type ShellPhoto = { id: string; name: string; type: string; size: number }
/** `failed`: picked items the shell couldn't get, such as a video iCloud didn't hand over. */
export type ShellPick = { files: ShellPhoto[]; failed: number }
/** `from`: what was tapped, in the page's CSS pixels, so an iPad's popover points at it. */
export type ShellShare = { url: string; text?: string; from?: { x: number; y: number; width: number; height: number } }

/** The upload limit; a bigger file isn't read in at all. */
const MAX = 1024 ** 3

export const shellPhotos = () => !!window.denShell?.photos

/**
 * Opens the shell's photo picker and hands each picked file over as a File, one by one as it arrives, the same as
 * the <input> would. The shell sends a file in base64 pieces (MediaBridge.swift says why). `warn` gets what went wrong.
 */
export async function pickShellPhotos(add: (files: File[]) => void, warn: (message: string) => void) {
  const shell = window.denShell!
  let picked: ShellPick
  try { picked = await shell.photos!() }
  catch (e) { warn((e as Error).message || 'Your photos couldn\'t be opened.'); return }
  if (picked.failed) warn(picked.failed === 1 ? 'One of those couldn\'t be added.' : `${picked.failed} of those couldn't be added.`)
  for (const photo of picked.files) {
    if (photo.size > MAX) { warn(`${photo.name} is over the 1 GB limit.`); continue }
    try { add([await readPhoto(photo, (id, offset) => shell.photoChunk!(id, offset))]) }
    catch { warn(`${photo.name} couldn't be added.`) }
  }
}

export async function readPhoto(photo: ShellPhoto, chunk: (id: string, offset: number) => Promise<string>) {
  const parts: BlobPart[] = []
  let offset = 0
  do {
    const piece = decode(await chunk(photo.id, offset))
    // A file that changed size under us would otherwise loop for ever.
    if (!piece.length && offset < photo.size) throw new Error('short read')
    // A Blob at once, so WebKit keeps the bytes and the page's own copy can go: a video never sits in the page whole.
    parts.push(new Blob([piece]))
    offset += piece.length
  } while (offset < photo.size)
  return new File(parts, photo.name, { type: photo.type })
}

function decode(base64: string) {
  const text = atob(base64)
  const bytes = new Uint8Array(text.length)
  for (let i = 0; i < text.length; i++) bytes[i] = text.charCodeAt(i)
  return bytes
}

/**
 * Shares a link: the iPhone app's share sheet, else the browser's. False when there is neither, so the caller can copy
 * the link instead. Rejects like `navigator.share` (an AbortError when the person closes the browser's sheet).
 */
export async function shareLink(data: { title?: string; text?: string; url: string }, from?: Element | null) {
  const shell = window.denShell?.share
  if (shell) {
    const r = from?.getBoundingClientRect()
    await shell({ url: data.url, text: data.text, from: r && { x: r.x, y: r.y, width: r.width, height: r.height } })
    return true
  }
  if (typeof navigator.share !== 'function') return false
  await navigator.share(data)
  return true
}

export const canShareLink = () => !!window.denShell?.share || typeof navigator.share === 'function'

// ---- The strip of recent photos at the top of the + sheet (Den Shell only) ----
// It is iOS's own picker, embedded as one row: it runs outside the app, so it needs no access to the library. Being a
// native view, it sits over the page, so it must never be seen anywhere but its slot in the sheet. The page tells the
// shell where the slot is only once the sheet has been still for two frames, and hides the strip the moment anything
// moves it (a pull, the sheet closing, the window changing size). Following a pull frame by frame over the bridge would
// lag the sheet and spill over what's around it; a strip that steps aside while the sheet moves can't. The slot keeps
// a plain card of the same size meanwhile, so nothing jumps.
//
// Picking: a tap ticks a photo and puts it in the tray, the same as Photos; another tap unticks it and takes it out.
// The strip is new every time the sheet opens, with ticks on the photos it gave that are still in the tray, so after a
// send (the tray empties) it starts clean.

type Rect = { x: number; y: number; width: number; height: number }
/** `session` is one opening of the strip. `selected`: the photos (by the library's ids) to show ticked. */
export type ShellStripCommand =
  | { type: 'stripShow'; session: number; rect: Rect; radius: number; selected: string[] }
  | { type: 'stripHide' | 'stripClose'; session: number }
/** A photo ticked (its file comes as a pick's would), unticked, or one iOS wouldn't hand over; `place`: the shell moved
 *  things itself (the keyboard), so the strip waits for where to go again. */
export type ShellStripEvent =
  | { type: 'added'; session: number; asset: string; file: ShellPhoto }
  | { type: 'removed'; session: number; asset: string }
  | { type: 'failed'; session: number; asset: string }
  | { type: 'place' }

/** The tray of the conversation the + sheet was opened from. */
export type StripTray = { files(): File[]; add(file: File): void; remove(file: File): void; warn(message: string): void }

export const shellStrip = () => !!window.denShell?.photoStrip

/** Which strip photo a tray file came from. */
const fromStrip = new WeakMap<File, string>()
/** Each open strip's tray, kept until the shell says its last photo has come in. `reading`: photos on their way. */
const sessions = new Map<number, { tray: StripTray; reading: Map<string, object> }>()
let session = 0, place: (() => void) | null = null

/** Starts a strip for `tray`: its events go there from now on. */
export function openStrip(tray: StripTray) {
  const shell = window.denShell!
  sessions.set(++session, { tray, reading: new Map() })
  shell.onPhotoStrip = (e) => {
    if (e.type === 'place') return place?.()
    const open = sessions.get(e.session)
    if (!open) return
    const { tray, reading } = open
    if (e.type === 'failed') return tray.warn('That photo couldn\'t be added.')
    if (e.type === 'removed') {
      reading.delete(e.asset)
      const file = tray.files().find((f) => fromStrip.get(f) === e.asset)
      if (file) tray.remove(file)
      return
    }
    if (e.file.size > MAX) return tray.warn(`${e.file.name} is over the 1 GB limit.`)
    // Unticked (or ticked again) while it was on its way: this copy isn't wanted.
    const mine = {}
    reading.set(e.asset, mine)
    readPhoto(e.file, (id, offset) => shell.photoChunk!(id, offset)).then((file) => {
      if (reading.get(e.asset) !== mine) return
      reading.delete(e.asset)
      fromStrip.set(file, e.asset)
      tray.add(file)
    }, () => tray.warn(`${e.file.name} couldn't be added.`))
  }
  return session
}

/** The ticks a new strip shows: its photos still in the tray. */
export const stripSelected = (tray: StripTray) => tray.files().flatMap((f) => fromStrip.get(f) ?? [])

/** The strip's slot in the + sheet (`node`, inside the sheet's popover). */
export function photoStrip(node: HTMLElement, tray: StripTray) {
  const shell = window.denShell!, id = openStrip(tray)
  const sheet = node.closest<HTMLElement>('[popover]')!
  const send = (command: ShellStripCommand) => shell.photoStrip!(command).catch(() => {})
  let shown = false, frame = 0, pulled = sheet.style.translate
  const hide = () => {
    cancelAnimationFrame(frame)
    if (shown) void send({ type: 'stripHide', session: id })
    shown = false
  }
  // Hidden at once; shown again where the slot is once it has stayed put for two frames, with the sheet open and
  // not pulled.
  const moved = () => {
    hide()
    let last = ''
    const settle = () => {
      const r = node.getBoundingClientRect(), now = [r.x, r.y, r.width, r.height].join()
      if (now !== last || !sheet.matches(':popover-open') || sheet.style.translate) { last = now; frame = requestAnimationFrame(settle); return }
      shown = true
      const radius = parseFloat(getComputedStyle(node).borderTopLeftRadius) || 0
      void send({ type: 'stripShow', session: id, rect: { x: r.x, y: r.y, width: r.width, height: r.height }, radius, selected: stripSelected(tray) })
    }
    frame = requestAnimationFrame(settle)
  }
  // pullDown (disclosure-popover.ts) moves the sheet with `translate`. Other style changes (its left and top) don't
  // move a sheet.
  const styled = new MutationObserver(() => { if (sheet.style.translate !== pulled) { pulled = sheet.style.translate; moved() } })
  styled.observe(sheet, { attributes: true, attributeFilter: ['style'] })
  const resized = new ResizeObserver(moved)
  resized.observe(node)
  // Before the sheet goes from the screen, not after: the strip mustn't outlast it by a frame.
  const closing = (e: Event) => { if ((e as ToggleEvent).newState === 'closed') hide() }
  sheet.addEventListener('beforetoggle', closing)
  addEventListener('resize', moved)
  place = moved
  moved()
  return {
    destroy() {
      hide()
      styled.disconnect(); resized.disconnect()
      sheet.removeEventListener('beforetoggle', closing)
      removeEventListener('resize', moved)
      if (place === moved) place = null
      // The shell answers once every photo ticked in this strip has come in.
      void send({ type: 'stripClose', session: id }).then(() => sessions.delete(id))
    },
  }
}
