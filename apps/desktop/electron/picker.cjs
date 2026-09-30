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

// Loopback is Windows-only in Chromium; elsewhere asking for it fails the whole capture.
function captureAnswer(source, { audio, audioRequested, platform }) {
  if (!source) return {}
  return { video: source, ...(audio && audioRequested && platform === 'win32' ? { audio: 'loopback' } : {}) }
}

function validatePickerFrame(frame, mainFrame, trusted) {
  return !!(frame && frame === mainFrame && trusted(frame.url))
}

module.exports = { pickSession, captureAnswer, validatePickerFrame }
