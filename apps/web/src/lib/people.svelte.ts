import { lab } from './theme-runtime'
import type { User } from './types'
import type { Store } from './store.svelte'

/** A person's own hue: their chosen profile colour, else a stable hash of their id. */
export function personHue(userId: string, user?: User): number {
  if (user?.accent) {
    const [, a, b] = lab(user.accent) as [number, number, number]
    return Math.round((Math.atan2(b, a) * 180 / Math.PI + 360) % 360)
  }
  return ([...userId].reduce((h, c) => Math.imul(h ^ c.charCodeAt(0), 16777619), 2166136261) >>> 0) % 360
}

/** The one profile card on screen, anchored to whatever opened it. */
class ProfileCard {
  userId = $state<string | null>(null)
  instance = $state<Store | null>(null)
  anchor: HTMLElement | null = null
  private closed = { at: 0, anchor: null as HTMLElement | null }
  open(userId: string, anchor: HTMLElement, instance: Store) {
    // Clicking the opener again should close it. Light dismiss has already closed the
    // popover on pointerdown by the time this click arrives, so treat that as done.
    if (this.closed.anchor === anchor && performance.now() - this.closed.at < 400) return
    if (this.userId === userId && this.anchor === anchor) return this.close()
    this.anchor = anchor; this.instance = instance; this.userId = userId
  }
  close() {
    if (this.userId) this.closed = { at: performance.now(), anchor: this.anchor }
    this.userId = null
  }
}
export const profileCard = new ProfileCard()
