// Keep tile and dock menus outside their clipping containers, including in PiP.
// `align` is which edge of the button the menu lines up with: its right edge by
// default, or its left edge for a button near the left of the screen.
export function disclosurePopover(node: HTMLElement, align: 'start' | 'end' = 'end') {
  const details = node.parentElement as HTMLDetailsElement
  const position = () => {
    if (!node.matches(':popover-open')) return
    const rect = details.getBoundingClientRect(), box = node.getBoundingClientRect()
    const viewport = node.ownerDocument.documentElement
    node.style.left = `${Math.max(8, Math.min(align === 'start' ? rect.left : rect.right - box.width, viewport.clientWidth - box.width - 8))}px`
    node.style.top = `${Math.max(8, Math.min(rect.top >= box.height + 8 ? rect.top - box.height - 4 : rect.bottom + 4, viewport.clientHeight - box.height - 8))}px`
  }
  const toggle = () => {
    if (details.open && !node.matches(':popover-open')) { node.showPopover(); position() }
    else if (!details.open) node.hidePopover()
  }
  const closed = (e: Event) => { if ((e as ToggleEvent).newState === 'closed') { details.open = false; node.style.translate = node.style.transition = '' } }
  const resize = new ResizeObserver(position)
  resize.observe(node)
  const view = node.ownerDocument.defaultView
  view?.addEventListener('resize', position)
  details.addEventListener('toggle', toggle); node.addEventListener('toggle', closed)
  const stopPull = pullDown(node)
  return { destroy() { resize.disconnect(); view?.removeEventListener('resize', position); details.removeEventListener('toggle', toggle); node.removeEventListener('toggle', closed); stopPull() } }
}

// A sheet along the bottom of a phone (app.css `.sheet`) follows a finger pulling it down and closes past a
// third of its height, or on a flick. Not while a list inside it is scrolled down: that pull scrolls the list.
// Pushed up, it gives a little and springs back.
function pullDown(node: HTMLElement) {
  let x = 0, y = 0, dy = 0, at = 0, speed = 0, tracking = false, pulling = false
  const scrolled = (target: EventTarget | null) => {
    for (let el = target as Element | null; el && el !== node; el = el.parentElement) if (el.scrollTop > 0) return true
    return node.scrollTop > 0
  }
  const start = (e: TouchEvent) => {
    // A second finger mid-pull: put the sheet back before starting over.
    if (pulling) cancel()
    tracking = e.touches.length === 1 && node.classList.contains('sheet') && matchMedia('(max-width: 600px)').matches && !scrolled(e.target)
    pulling = false; dy = speed = 0; x = e.touches[0]!.clientX; y = e.touches[0]!.clientY; at = e.timeStamp
  }
  const move = (e: TouchEvent) => {
    if (!tracking) return
    const touch = e.touches[0]!, next = touch.clientY - y
    if (!pulling) {
      // Up or sideways is the sheet's own scrolling.
      if (next < -4 || Math.abs(touch.clientX - x) > Math.abs(next)) { tracking = false; return }
      if (next < 8) return
      pulling = true; node.style.transition = 'none'
    }
    e.preventDefault()
    speed = (next - dy) / Math.max(1, e.timeStamp - at); at = e.timeStamp; dy = next
    node.style.translate = `0 ${dy > 0 ? dy : dy / 4}px`
  }
  const end = (e?: Event) => {
    if (!pulling) return
    tracking = pulling = false
    // A flick counts only if the finger was still moving when it let go.
    if (e && e.timeStamp - at > 80) speed = 0
    node.style.transition = 'translate var(--t) var(--ease-out)'
    if (dy > node.offsetHeight / 3 || (dy > 24 && speed > 0.5)) {
      node.style.translate = '0 100%'
      setTimeout(() => node.hidePopover(), 160)
    } else node.style.translate = ''
  }
  // iOS took the touch away (a call, a system gesture): put the sheet back.
  const cancel = () => { dy = speed = 0; end() }
  node.addEventListener('touchstart', start, { passive: true })
  node.addEventListener('touchmove', move, { passive: false })
  node.addEventListener('touchend', end); node.addEventListener('touchcancel', cancel)
  return () => {
    node.removeEventListener('touchstart', start); node.removeEventListener('touchmove', move)
    node.removeEventListener('touchend', end); node.removeEventListener('touchcancel', cancel)
  }
}
