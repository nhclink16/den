// Inside Den Shell (the iPhone app) notifications come through Apple's push service. The shell owns the iPhone's
// side: the permission, the device token, the app badge and taps. This page owns the account's side: it gives the
// server the token for whoever is signed in, keeps the badge at the server's count and opens what a tap points at.
// Browsers and shells from before push have no `denShell.push`, and none of this runs.
import { shell } from './shell-call'
import { instances } from './store.svelte'
import { goToMessage, lookingAt } from './notify.svelte'
import type { ChannelReadState, RegisterDevice } from './types'

export type PushStatus = { permission: 'granted' | 'denied' | 'undecided'; token?: string; environment: 'sandbox' | 'production'; appVersion: string }
export type PushTap = { channelId: string; messageId?: string; threadId?: string }
export type PushCommand = { type: 'register' | 'status' | 'take' | 'signedOut' | 'settings' } | { type: 'badge'; count: number }
/** From the shell. `present` asks whether a notification arriving while Den is open is already on screen. */
export type PushEvent = ({ type: 'status' } & PushStatus) | { type: 'tap' } | { type: 'present'; channelId: string; threadId?: string }

/** `connected`: the server has this iPhone's token for the signed-in account. */
export const push = $state({ available: typeof shell?.push === 'function', status: null as PushStatus | null, connected: false })

let sent = ''
let sending = ''
// Numbers each registration, and moves on at sign-out, so a late reply can't stand for a newer one.
let attempt = 0
// Set at sign-out, cleared once the server has the next account's token. If that sign-out never reached the
// server, the old account's registration still holds this token there.
const SIGNED_OUT = 'den.push.signedOut'
const ask = <T>(command: PushCommand) => shell!.push!(command) as Promise<T>

/** After sign-in and whenever Den comes back to the front. iOS shows its prompt only the first time; after that this
 *  registers again (Apple asks for that at every launch), and opens a notification tapped while the page was away. */
export async function pushStart() {
  if (!push.available) return
  const status = await ask<PushStatus>({ type: 'register' }).catch(() => null)
  if (status) await receive(status)
  // iOS may have changed the badge from a notification while Den was away.
  if (instances.active.ready) pushBadge(instances.active.readState.values())
  await openTap()
}

async function receive(status: PushStatus) {
  push.status = status
  const source = instances.active
  if (status.permission !== 'granted' || !status.token || !source.ready || !source.me) return
  // Once per account and token for this page; the server keeps it until that sign-in ends.
  const key = `${source.me.id} ${status.environment} ${status.token}`
  if (key === sent || key === sending) return
  const mine = ++attempt
  sending = key
  const device: RegisterDevice = { platform: 'ios', token: status.token, app_version: status.appVersion, purpose: 'alert', environment: status.environment }
  try {
    await source.api.post('/devices', device)
    if (mine !== attempt) return
    sent = key
    push.connected = true
    localStorage.removeItem(SIGNED_OUT)
  } catch {
    if (mine !== attempt) return
    push.connected = false
    // Apple would deliver the old account's alerts to this token again: switch push off until a later try works.
    if (localStorage.getItem(SIGNED_OUT)) void ask({ type: 'signedOut' }).catch(() => {})
  } finally {
    if (mine === attempt) sending = ''
  }
}

async function openTap() {
  const source = instances.active
  if (!source.ready) return
  const lifetime = source.drafts, token = lifetime.token
  const tap = await ask<PushTap | null>({ type: 'take' }).catch(() => null)
  // Signed out (and maybe someone else in) while the shell answered: the tap belonged to the old account.
  if (tap && lifetime.holds(token)) await goToMessage(source, tap.channelId, tap.messageId)
}

/** Installed while someone is signed in. Before that a tap waits in the shell, and `present` gets the full alert. */
export function listenForPush() {
  if (!push.available) return () => {}
  window.denShell!.onPush = (event) => {
    if (event.type === 'status') void receive(event)
    else if (event.type === 'tap') void openTap()
    else if (event.type === 'present') return instances.active.ready ? lookingAt(instances.active, event.channelId, event.threadId) : null
  }
  return () => { window.denShell!.onPush = null }
}

/** The app icon's number: the same count the server puts in each notification. Always sent, because iOS also
 *  sets it from notifications the page never sees. */
export function pushBadge(states: Iterable<ChannelReadState>) {
  if (!push.available) return
  let count = 0
  for (const s of states) count += s.notification_count
  void ask({ type: 'badge', count }).catch(() => {})
}

/** At sign-out: clears the badge and notifications, and forgets the token so the next account registers afresh. */
export function pushSignedOut() {
  if (!push.available) return
  sent = sending = ''
  attempt++
  push.connected = false
  localStorage.setItem(SIGNED_OUT, '1')
  void ask({ type: 'signedOut' }).catch(() => {})
}

export function pushSettings() {
  if (push.available) void ask({ type: 'settings' }).catch(() => {})
}
