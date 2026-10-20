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

/** `connected`: the server has this iPhone's token for the signed-in account. `asking`: Den's own question is up
 *  (PushAsk.svelte). */
export const push = $state({ available: typeof shell?.push === 'function', status: null as PushStatus | null, connected: false, asking: false })

let sent = ''
let sending = ''
// Numbers each registration, and moves on at sign-out, so a late reply can't stand for a newer one.
let attempt = 0
// Set at sign-out, cleared once the server has the next account's token. If that sign-out never reached the
// server, the old account's registration still holds this token there.
const SIGNED_OUT = 'den.push.signedOut'
// "Not Now" on Den's question, for this iPhone. Settings > Notifications still has Turn on.
const NOT_NOW = 'den.push.notNow'
// Registrations under way. The first one brings up iOS's prompt, so while any is out Den doesn't ask on top of it.
let registering = 0
// Numbers the checks of iOS's answer. Signing out, or a registration starting or finishing, makes the ones in flight
// stale. `session` moves on at sign-out, so an old account's registration finishing can't touch the next one's checks.
let checks = 0
let session = 0
const ask = <T>(command: PushCommand) => shell!.push!(command) as Promise<T>

/** Registers this iPhone for whoever is signed in, and opens a notification tapped while the page was away. Apple
 *  asks for a registration at every launch. iOS shows its own prompt only once, the first time, so after sign-in and
 *  whenever Den comes back (`askFirst`) Den asks in its own words first, and only a yes brings up iOS's prompt. */
export async function pushStart({ askFirst = false } = {}) {
  if (!push.available) return
  const check = ++checks
  const now = askFirst ? await ask<PushStatus>({ type: 'status' }).catch(() => null) : null
  // The reply speaks for an older moment: someone signed out (it would register them again), or iOS's prompt was
  // answered since (an "undecided" would bring the question back).
  if (askFirst && check !== checks) return
  if (now?.permission === 'undecided') {
    push.status = now
    push.asking = !registering && !localStorage.getItem(NOT_NOW)
  } else {
    push.asking = false
    const mine = session
    registering++
    const status = await ask<PushStatus>({ type: 'register' }).catch(() => null)
    registering--
    if (mine !== session) return
    checks++
    if (status) await receive(status)
  }
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

/** The answer to Den's question. Turn On brings up iOS's prompt. */
export function pushAnswer(on: boolean) {
  push.asking = false
  if (on) void pushStart()
  else localStorage.setItem(NOT_NOW, '1')
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
  checks++
  session++
  push.connected = false
  push.asking = false
  localStorage.setItem(SIGNED_OUT, '1')
  void ask({ type: 'signedOut' }).catch(() => {})
}

export function pushSettings() {
  if (push.available) void ask({ type: 'settings' }).catch(() => {})
}
