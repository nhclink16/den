// One capture request's sources. Every list shown to the renderer, including the
// thumbnail refreshes, is remembered, so a window that appears while the picker is
// open can be shared, and nothing the renderer invents can be.
function pickSession() {
  const sources = new Map()
  return {
    offer(list) { for (const s of list) sources.set(s.id, s); return list },
    // The source to capture, null for a cancel, false for an id that was never offered.
    choose(sourceId) { return sourceId == null ? null : sources.get(sourceId) ?? false },
  }
}

// Helper and overlay windows (tray helpers, GPU overlays) capture as empty or pure black.
// One lit pixel keeps a window, so a dark terminal with any text survives.
function blankThumbnail(image) {
  if (image.isEmpty()) return true
  const px = image.toBitmap() // BGRA
  for (let i = 0; i + 3 < px.length; i += 4) if (px[i + 3] > 16 && Math.max(px[i], px[i + 1], px[i + 2]) > 24) return false
  return true
}
const visibleSources = list => list.filter(s => s.id.startsWith('screen:') || !blankThumbnail(s.thumbnail))

// Loopback is Windows-only in Chromium; elsewhere asking for it fails the whole capture.
function captureAnswer(source, { audio, audioRequested, platform }) {
  if (!source) return {}
  return { video: source, ...(audio && audioRequested && platform === 'win32' ? { audio: 'loopback' } : {}) }
}

// One picker is open at a time, and every capture request gets exactly one answer.
// The older request is ended where the newer one is installed, after listing sources,
// so two requests that overlap while listing cannot overwrite each other.
function picker({ list, show, dismiss, platform, timeoutMs = 120_000 }) {
  let open = null
  const end = (req, result) => { if (open === req) open = null; clearTimeout(req.timer); req.resolve(result) }
  return {
    async request(audioRequested) {
      const sources = await list()
      const result = await new Promise(resolve => {
        if (open) { const old = open; end(old, null); dismiss(old.id) }
        const req = open = { id: `pick-${Date.now()}-${Math.random().toString(36).slice(2)}`, pick: pickSession(), resolve }
        req.pick.offer(sources)
        req.timer = setTimeout(() => { end(req, null); dismiss(req.id) }, timeoutMs)
        show(req.id, sources, audioRequested)
      })
      return captureAnswer(result?.source, { audio: result?.audio, audioRequested, platform })
    },
    choose(requestId, sourceId, audio) {
      if (!open || open.id !== requestId) return
      const req = open, source = req.pick.choose(sourceId)
      end(req, source ? { source, audio: !!audio } : null)
    },
    // A fresh list for the open picker, or null once it has ended, so the renderer closes.
    async refresh() {
      const req = open
      if (!req) return null
      const sources = await list()
      return open === req ? req.pick.offer(sources) : null
    },
  }
}

function validatePickerFrame(frame, mainFrame, trusted) {
  return !!(frame && frame === mainFrame && trusted(frame.url))
}

module.exports = { pickSession, picker, captureAnswer, validatePickerFrame, visibleSources }
