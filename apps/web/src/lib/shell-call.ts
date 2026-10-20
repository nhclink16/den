// Inside Den Shell (the iPhone app) the call runs natively, so it survives a locked phone. The page still
// draws it: the shell sends the whole call state each time something changes, and this file turns that
// into what the call view reads. Everywhere else `window.denShell` is missing and none of this runs.
import type { ConnectionQuality } from 'livekit-client'
import type { CallParticipant } from './call.svelte'
import type { PushCommand, PushEvent } from './shell-push.svelte'
import type { Haptic } from './haptics'
import type { ShellPhoto, ShellShare } from './shell-media'

export type ShellPerson = { identity: string; name: string; local: boolean; speaking: boolean; muted: boolean; quality: string; camera: boolean; screen: boolean }
/** `revision` grows with every snapshot; `micSeq`/`soundSeq` echo the last mute/sound command the shell applied. */
export type ShellCall = {
  revision: number; phase: 'connecting' | 'connected' | 'reconnecting' | 'ended'
  channelId: string; micSeq: number; soundSeq: number; micOn: boolean; soundOff: boolean; otherDevices: number; error?: string
  participants: ShellPerson[]
}
/** The shell has no call (a revision alone). Ordered like snapshots, so a late one can't end a newer call. */
export type ShellNoCall = { revision: number; phase?: undefined }
export type ShellUpdate = ShellCall | ShellNoCall
export type ShellCommand =
  | { type: 'join'; url: string; token: string; channelId: string; title: string; micOn: boolean }
  | { type: 'leave' } | { type: 'state' } | { type: 'mute'; muted: boolean; seq: number } | { type: 'sound'; off: boolean; seq: number }
declare global {
  // `push` and `onPush` came later (shell-push.svelte.ts), then `haptic` (haptics.ts), then `photos`, `photoChunk`
  // and `share` (shell-media.ts): older shells don't have them.
  interface Window {
    denShell?: {
      call(command: ShellCommand): Promise<ShellUpdate | null>; onCall: ((update: ShellUpdate) => void) | null
      push?(command: PushCommand): Promise<unknown>; onPush?: ((event: PushEvent) => unknown) | null
      haptic?(kind: Haptic): void
      photos?(): Promise<ShellPhoto[]>; photoChunk?(id: string, offset: number): Promise<string>
      share?(data: ShellShare): Promise<void>
    }
  }
}

export const shell = typeof window !== 'undefined' ? window.denShell : undefined

const accountId = (identity: string) => identity.split(':', 1)[0]!

/** The same people the web engine would list, so the dock, the grid and the people count read them alike. */
export function shellParticipants(call: ShellCall, name: (userId: string) => string, djName: string): CallParticipant[] {
  const people = call.participants
  return people.map((p) => {
    const devices = people.filter((m) => accountId(m.identity) === accountId(p.identity)).sort((a, b) => a.identity.localeCompare(b.identity))
    const music = p.identity === `den-dj-${call.channelId}`
    return {
      id: p.identity, userId: accountId(p.identity),
      device: devices.length > 1 ? p.local ? 'This device' : `Device ${devices.indexOf(p) + 1}` : '',
      music, name: music ? p.name || djName : name(accountId(p.identity)), local: p.local,
      speaking: p.speaking, muted: p.muted, screens: [], quality: p.quality as ConnectionQuality,
      // The phone can't show streams yet; the tile says so instead.
      unseen: p.screen ? 'screen' : p.camera ? 'camera' : undefined,
    }
  })
}
