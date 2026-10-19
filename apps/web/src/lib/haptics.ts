// Den Shell (the iPhone app) plays a small tap at a few moments: a message sent, a reaction added, a call joined
// or left, something that went wrong. Few on purpose: never on scroll, typing, messages arriving or every tap.
// Everywhere else `window.denShell.haptic` is missing and none of this does anything.
export type Haptic = 'light' | 'medium' | 'selection' | 'warning'

export function haptic(kind: Haptic) { window.denShell?.haptic?.(kind) }

/** A tick when a switch flips or a segmented control picks something else, as iOS's own controls do. */
export function listenForSelection() {
  if (!window.denShell?.haptic) return () => {}
  const changed = (e: Event) => { if ((e.target as Element).matches('input.toggle, .segments input')) haptic('selection') }
  // Read before the click's handler runs, while the button still says whether it was already the choice.
  const pressed = (e: MouseEvent) => {
    const button = (e.target as Element).closest<HTMLButtonElement>('.segments button')
    if (button && !button.disabled && button.getAttribute('aria-pressed') === 'false') haptic('selection')
  }
  document.addEventListener('change', changed, true)
  document.addEventListener('click', pressed, true)
  return () => { document.removeEventListener('change', changed, true); document.removeEventListener('click', pressed, true) }
}
