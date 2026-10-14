// What the server shows only once: a new API key or agent token, and new invite links.
// It stays while Settings is open, across its sections, and goes when you leave Settings.
import type { Invite } from './types'

export const shownOnce = $state<{ reveal: { label: string; token: string } | null; invites: Invite[] }>({ reveal: null, invites: [] })
export function forgetShownOnce() { shownOnce.reveal = null; shownOnce.invites = [] }
