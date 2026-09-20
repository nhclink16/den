// Spotify Jams. A Jam is a share link pinned to a room, not a message.
import type { Store } from './store.svelte'
import type { Jam } from './types'

/** Mirrors jam_url() in crates/den-server/src/jams.rs so the UI offers only links
 *  the server would accept. The server is still the one that validates. */
export const JAM_LINK = /^https:\/\/(?:open\.spotify\.com\/jam\/|spotify\.link\/)[A-Za-z0-9_-]{1,64}(?:[?#]|$)/i

export const isJamLink = (text: string) => JAM_LINK.test(text.trim())

/** Pin a Jam to a room. The server ends any Jam already live there. */
export async function startJam(owner: Store, channelId: string, url: string) {
  return owner.mutateJam(channelId, () => owner.api.post<Jam>(`/rooms/${channelId}/jam`, { url: url.trim() }))
}

export async function joinJam(owner: Store, channelId: string) {
  return owner.mutateJam(channelId, () => owner.api.post<Jam>(`/rooms/${channelId}/jam/join`, {}))
}

export async function endJam(owner: Store, channelId: string) {
  return owner.mutateJam(channelId, async () => { await owner.api.del(`/rooms/${channelId}/jam`); return null })
}
