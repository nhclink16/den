/** Native modality keeps Tab and pointer interaction inside the topmost dialog. */
export function modal(node: HTMLDialogElement) {
  const opener = document.activeElement instanceof HTMLElement ? document.activeElement : null
  node.showModal()
  const initial = node.querySelector<HTMLElement>('[data-initial-focus]') || node
  initial.focus()
  return { destroy() { node.close(); if (opener?.isConnected) opener.focus() } }
}

export function outsideDialog(event: MouseEvent): boolean {
  if (event.target !== event.currentTarget) return false
  const box = (event.currentTarget as HTMLDialogElement).getBoundingClientRect()
  return event.clientX < box.left || event.clientX > box.right || event.clientY < box.top || event.clientY > box.bottom
}
