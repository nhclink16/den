export type ShareSurface = 'monitor' | 'window' | 'browser'
export function shareSource(raw = '', surface?: string): { label: string; surface: ShareSurface } {
  const kind = surface === 'browser' || surface === 'window' || surface === 'monitor' ? surface
    : /^window\s*:/i.test(raw) ? 'window' : /^((web-contents|tab)\s*:)/i.test(raw) ? 'browser' : 'monitor'
  const label = raw.replace(/[\u0000-\u001f\u007f]/g, '').trim()
  // Chromium/Electron source handles are identifiers, not application names.
  const opaque = /^(?:(?:screen|window|monitor|web-contents|tab)\s*[:\-]\s*)?(?:-?\d+(?::-?\d+)*|0x[\da-f]+)$/i.test(label)
    || /^[\da-f]{8}-[\da-f-]{27,}$/i.test(label)
  return { surface: kind, label: (!label || opaque ? kind === 'window' ? 'Window' : kind === 'browser' ? 'Browser tab' : 'Screen' : label).slice(0, 100) }
}
