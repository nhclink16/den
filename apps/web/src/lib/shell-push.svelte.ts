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
let badge = -1
const ask = <T>(command: PushCommand) => shell!.push!(command) as Promise<T>

/** After sign-in and whenever Den comes back to the front. iOS shows its prompt only the first time; after that this
 *  registers again (Apple asks for that at every launch), and opens a notification tapped while the page was away. */
export async function pushStart() {
  if (!push.available) return
  const status = await ask<PushStatus>({ type: 'register' }).catch(() => null)
  if (status) await receive(status)
  await openTap()
}

async function receive(status: PushStatus) {
  push.status = status
  const source = instances.active
  if (status.permission !== 'granted' || !status.token || !source.ready || !source.me) return
  // Once per account and token for this page; the server keeps it until that sign-in ends.
  const key = `${source.me.id} ${status.environment} ${status.token}`
  if (key === sent || key === sending) return
  sending = key
  const device: RegisterDevice = { platform: 'ios', token: status.token, app_version: status.appVersion, purpose: 'alert', environment: status.environment }
  try {
    await source.api.post('/devices', device)
    if (sending === key) { sent = key; push.connected = true }
  } catch {
    if (sending === key) push.connected = false
  } finally {
    if (sending === key) sending = ''
  }
}

async function openTap() {
  const source = instances.active
  if (!source.ready) return
  const tap = await ask<PushTap | null>({ type: 'take' }).catch(() => null)
  if (tap) await goToMessage(source, tap.channelId, tap.messageId)
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

/** The app icon's number: the same count the server puts in each notification. */
export function pushBadge(states: Iterable<ChannelReadState>) {
  if (!push.available) return
  let count = 0
  for (const s of states) count += s.notification_count
  if (count === badge) return
  badge = count
  void ask({ type: 'badge', count }).catch(() => {})
}

/** At sign-out: clears the badge and notifications, and forgets the token so the next account registers afresh. */
export function pushSignedOut() {
  if (!push.available) return
  sent = sending = ''
  badge = -1
  push.connected = false
  void ask({ type: 'signedOut' }).catch(() => {})
}

export function pushSettings() {
  if (push.available) void ask({ type: 'settings' }).catch(() => {})
}
