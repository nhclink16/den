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
  const closed = (e: Event) => { if ((e as ToggleEvent).newState === 'closed') details.open = false }
  const resize = new ResizeObserver(position)
  resize.observe(node)
  const view = node.ownerDocument.defaultView
  view?.addEventListener('resize', position)
  details.addEventListener('toggle', toggle); node.addEventListener('toggle', closed)
  return { destroy() { resize.disconnect(); view?.removeEventListener('resize', position); details.removeEventListener('toggle', toggle); node.removeEventListener('toggle', closed) } }
}
