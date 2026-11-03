// Inside Den Shell (the iPhone app) the call runs natively, so it survives a locked phone. The page still
// draws it: the shell sends the whole call state each time something changes, and this file turns that
// into what the call view reads. Everywhere else `window.denShell` is missing and none of this runs.
import type { ConnectionQuality } from 'livekit-client'
import type { CallParticipant } from './call.svelte'
import type { PushCommand, PushEvent } from './shell-push.svelte'
import type { Haptic } from './haptics'
import type { ShellPick, ShellShare, ShellStripCommand, ShellStripEvent } from './shell-media'
import type { ShellVideoCommand, ShellVideoEvent, ShellVideoReply } from './shell-video'
import { shareSource } from './share-source.ts'

/** `screens` (shells that draw video): each screen share, by its track name, with the label the sharer's app gave it. */
export type ShellPerson = {
  identity: string; name: string; local: boolean; speaking: boolean; muted: boolean; quality: string; camera: boolean; screen: boolean
  screens?: { name: string; label: string; surface: string }[]
}
/** `revision` grows with every snapshot; `micSeq`/`soundSeq`/`cameraSeq` echo the last command of each the shell applied.
 *  `cameraOn`/`cameraSeq` come from shells that draw video. */
export type ShellCall = {
  revision: number; phase: 'connecting' | 'connected' | 'reconnecting' | 'ended'
  channelId: string; micSeq: number; soundSeq: number; micOn: boolean; soundOff: boolean; otherDevices: number; error?: string
  cameraOn?: boolean; cameraSeq?: number
  participants: ShellPerson[]
}
/** The shell has no call (a revision alone). Ordered like snapshots, so a late one can't end a newer call. */
export type ShellNoCall = { revision: number; phase?: undefined }
export type ShellUpdate = ShellCall | ShellNoCall
export type ShellCommand =
  | { type: 'join'; url: string; token: string; channelId: string; title: string; micOn: boolean }
  | { type: 'leave' } | { type: 'state' } | { type: 'mute'; muted: boolean; seq: number } | { type: 'sound'; off: boolean; seq: number }
  | { type: 'camera'; on: boolean; seq: number } | { type: 'flip' }
declare global {
  // `push` and `onPush` came later (shell-push.svelte.ts), then `haptic` (haptics.ts), then `photos`, `photoChunk`
  // and `share`, then `photoStrip` and `onPhotoStrip` (shell-media.ts), then `video` and `onVideo` (shell-video.ts), which
  // also bring the `camera` and `flip` calls: older shells don't have them.
  interface Window {
    denShell?: {
      call(command: ShellCommand): Promise<ShellUpdate | null>; onCall: ((update: ShellUpdate) => void) | null
      push?(command: PushCommand): Promise<unknown>; onPush?: ((event: PushEvent) => unknown) | null
      haptic?(kind: Haptic): void
      photos?(): Promise<ShellPick>; photoChunk?(id: string, offset: number): Promise<string>
      share?(data: ShellShare): Promise<void>
      photoStrip?(command: ShellStripCommand): Promise<void>; onPhotoStrip?: ((event: ShellStripEvent) => void) | null
      video?(command: ShellVideoCommand): Promise<ShellVideoReply>; onVideo?: ((event: ShellVideoEvent) => void) | null
    }
  }
}

export const shell = typeof window !== 'undefined' ? window.denShell : undefined
/** This shell draws cameras and shares itself (CallTile leaves it a slot), and can send your camera. */
export const shellVideo = !!shell?.video

const accountId = (identity: string) => identity.split(':', 1)[0]!

/** The same people the web engine would list, so the dock, the grid and the people count read them alike.
 *  `video`: the shell draws cameras and shares (`nativeCamera`, native shares); an older one can't, so the tile says so. */
export function shellParticipants(call: ShellCall, name: (userId: string) => string, djName: string, video = false): CallParticipant[] {
  const people = call.participants
  return people.map((p) => {
    const devices = people.filter((m) => accountId(m.identity) === accountId(p.identity)).sort((a, b) => a.identity.localeCompare(b.identity))
    const music = p.identity === `den-dj-${call.channelId}`
    return {
      id: p.identity, userId: accountId(p.identity),
      device: devices.length > 1 ? p.local ? 'This device' : `Device ${devices.indexOf(p) + 1}` : '',
      music, name: music ? p.name || djName : name(accountId(p.identity)), local: p.local,
      speaking: p.speaking, muted: p.muted, quality: p.quality as ConnectionQuality,
      ...video ? {
        nativeCamera: p.camera || undefined,
        screens: (p.screens ?? []).map((s) => ({ name: s.name, ...shareSource(s.label, s.surface), native: true })),
      } : {
        // This shell can't show streams; the tile says so instead.
        screens: [], unseen: p.screen ? 'screen' as const : p.camera ? 'camera' as const : undefined,
      },
    }
  })
}
