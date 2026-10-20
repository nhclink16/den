// Den Shell (the iPhone app) picks photos with iOS's own picker, in one step: the page's <input> shows WebKit's
// "Photo Library / Take Photo / Choose File" menu first. It also shows iOS's share sheet, which `navigator.share`
// didn't bring up there. Shells from before this, browsers and the desktop keep the <input> and `navigator.share`.

/** A picked photo or video, kept by the shell until the page has read it. */
export type ShellPhoto = { id: string; name: string; type: string; size: number }
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
  let picked: ShellPhoto[]
  try { picked = await shell.photos!() }
  catch (e) { warn((e as Error).message || 'Your photos couldn\'t be opened.'); return }
  for (const photo of picked) {
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
    parts.push(piece)
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
