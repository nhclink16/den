// Den Shell (the iPhone app) passes the iPhone's Text Size as how much larger than default it makes body text, and
// app.css multiplies the five type tokens by it. Den follows it up to 1.3, where rows and bubbles still hold their
// shape; iOS's XXXL and every larger accessibility size land there. Never below the default: Den's smallest sizes
// are already the smallest it reads well at. Everywhere else `window.denShell.textScale` is missing and none of this runs.

export const MAX_TEXT_SCALE = 1.3

/** What the page uses for a value from the shell: anything that isn't a number reads as the default. */
export function textScale(value: unknown): number {
  if (typeof value !== 'number' || !Number.isFinite(value)) return 1
  return Math.round(Math.min(MAX_TEXT_SCALE, Math.max(1, value)) * 1000) / 1000
}

/** Asks the shell once, then follows each change it sends. */
export function followTextSize() {
  const shell = window.denShell
  if (!shell?.textScale) return
  const set = (value: unknown) => document.documentElement.style.setProperty('--text-scale', String(textScale(value)))
  shell.onTextScale = set
  shell.textScale().then(set, () => {})
}
