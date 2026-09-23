import type { UserStatus } from './types'

/** A status that is still current. Expiry is enforced lazily by the server, so an
 *  open client must not keep showing one whose time has passed. */
export function liveStatus(status: UserStatus | null | undefined, now = Date.now()): UserStatus | null {
  if (!status || !(status.emoji || status.text)) return null
  if (status.expires_at && status.expires_at * 1000 <= now) return null
  return status
}
