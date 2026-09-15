// Keep tile and dock menus outside their clipping containers, including in PiP.
export function disclosurePopover(node: HTMLElement) {
  const details = node.parentElement as HTMLDetailsElement
  const toggle = () => {
    if (details.open && !node.matches(':popover-open')) {
      node.showPopover()
      const rect = details.getBoundingClientRect(), box = node.getBoundingClientRect()
      const viewport = node.ownerDocument.documentElement
      node.style.left = `${Math.max(8, Math.min(rect.right - box.width, viewport.clientWidth - box.width - 8))}px`
      node.style.top = `${Math.max(8, Math.min(rect.top >= box.height + 8 ? rect.top - box.height - 4 : rect.bottom + 4, viewport.clientHeight - box.height - 8))}px`
    } else if (!details.open) node.hidePopover()
  }
  const closed = (e: Event) => { if ((e as ToggleEvent).newState === 'closed') details.open = false }
  details.addEventListener('toggle', toggle); node.addEventListener('toggle', closed)
  return { destroy() { details.removeEventListener('toggle', toggle); node.removeEventListener('toggle', closed) } }
}
