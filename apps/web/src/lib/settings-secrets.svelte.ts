// What the server shows only once: a new API key or agent token, and new invite links.
// It stays while Settings is open, across its sections, and goes when you leave Settings.
import type { Invite } from './types'

export const shownOnce = $state<{ reveal: { label: string; token: string } | null; invites: Invite[] }>({ reveal: null, invites: [] })
// Each Settings visit gets a number. A reply that lands after you left Settings belongs
// to an old visit and is dropped, so a secret never shows on a later visit (or account).
let visit = 0
export const settingsVisit = () => visit
export function forgetShownOnce() { visit++; shownOnce.reveal = null; shownOnce.invites = [] }
