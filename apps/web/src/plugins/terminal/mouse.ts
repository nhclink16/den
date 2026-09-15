import type { Terminal } from 'ghostty-web'

// Ghostty owns DEC mode parsing. Query it for every event, including mode changes
// split across output chunks. Encoding follows XTerm Control Sequences, Mouse Tracking.
export function mouseInput(host: HTMLElement, term: Terminal, send: (bytes: Uint8Array) => void, enabled: () => boolean) {
  const canvas = term.renderer!.getCanvas()
  const abort = new AbortController()
  let pressed = -1
  let lastCell = ''
  const tracking = () => enabled() && term.hasMouseTracking()
  const focused = () => host.contains(document.activeElement)
  const report = (e: MouseEvent, button: number, release = false) => {
    const rect = canvas.getBoundingClientRect()
    const x = Math.max(1, Math.min(term.cols, Math.floor((e.clientX - rect.left) * term.cols / rect.width) + 1))
    const y = Math.max(1, Math.min(term.rows, Math.floor((e.clientY - rect.top) * term.rows / rect.height) + 1))
    // X10 reports only button presses, without modifiers.
    if (!term.getMode(9)) button |= (e.shiftKey ? 4 : 0) | (e.altKey || e.metaKey ? 8 : 0) | (e.ctrlKey ? 16 : 0)
    const cell = `${button}:${x}:${y}`
    if (e.type === 'mousemove' && cell === lastCell) return
    lastCell = cell
    if (term.getMode(1006) || term.getMode(1016)) {
      const px = term.getMode(1016) ? Math.max(1, Math.round(e.clientX - rect.left) + 1) : x
      const py = term.getMode(1016) ? Math.max(1, Math.round(e.clientY - rect.top) + 1) : y
      send(new TextEncoder().encode(`\x1b[<${button};${px};${py}${release ? 'm' : 'M'}`))
    } else {
      if (release) button = (button & 28) | 3
      if (term.getMode(1015)) send(new TextEncoder().encode(`\x1b[${button + 32};${x};${y}M`))
      else if (term.getMode(1005)) send(new TextEncoder().encode(`\x1b[M${String.fromCharCode(button + 32, x + 32, y + 32)}`))
      // Legacy coordinates are bytes, not UTF-8. Do not wrap at column 224.
      else if (x <= 223 && y <= 223) send(new Uint8Array([27, 91, 77, button + 32, x + 32, y + 32]))
    }
  }
  const consume = (e: Event) => { e.preventDefault(); e.stopImmediatePropagation() }
  const on = (target: EventTarget, event: string, fn: (e: MouseEvent) => void) => target.addEventListener(event, fn as EventListener, { capture: true, signal: abort.signal })
  // Capture on the parent before Ghostty's selection and scrollbar handlers.
  on(host.parentElement!, 'mousedown', e => {
    if (e.target !== canvas || e.button > 2 || e.shiftKey || !tracking()) return
    term.focus(); pressed = e.button; lastCell = ''; consume(e); report(e, pressed)
  })
  on(document, 'mousemove', e => {
    if (!tracking() || !focused() || (pressed < 0 && (e.shiftKey || e.target !== canvas))) return
    if (!term.getMode(1003) && !(term.getMode(1002) && pressed >= 0)) return
    consume(e); report(e, (pressed >= 0 ? pressed : 3) | 32)
  })
  on(document, 'mouseup', e => {
    const button = pressed; pressed = -1; lastCell = ''
    if (button < 0 || !tracking() || !focused()) return
    consume(e); if (!term.getMode(9)) report(e, button, true)
  })
  on(canvas, 'click', e => { if (!e.shiftKey && tracking() && focused()) consume(e) })
  on(canvas, 'contextmenu', e => { if (!e.shiftKey && tracking() && focused()) consume(e) })
  host.addEventListener('focusout', () => { pressed = -1; lastCell = '' }, { signal: abort.signal })
  term.attachCustomWheelEventHandler(e => {
    if (!tracking() || e.shiftKey) {
      // Ghostty's default alternate-screen handler sends arrows. Off means scrollback.
      const height = term.renderer!.getMetrics().height
      term.scrollLines(Math.sign(e.deltaY) * Math.max(1, Math.round(Math.abs(e.deltaY) / (e.deltaMode === 0 ? height : 1))))
      return true
    }
    if (!focused() || term.getMode(9)) return true
    const delta = e.deltaY || e.deltaX
    if (!delta) return true
    const button = e.deltaY ? (delta < 0 ? 64 : 65) : (delta < 0 ? 66 : 67)
    const count = Math.min(5, Math.max(1, Math.round(Math.abs(delta) / (e.deltaMode === 0 ? 33 : 1))))
    for (let i = 0; i < count; i++) report(e, button)
    return true
  })
  return () => { abort.abort(); term.attachCustomWheelEventHandler(undefined) }
}
