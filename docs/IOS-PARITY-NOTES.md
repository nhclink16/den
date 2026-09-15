# iOS API parity — audit and profile updates

Audited the native app's existing surfaces against the live production schema and the
Rust source in this worktree. Scope was current native surfaces only: no music queue,
sound pack, profile editor, voice preferences or terminal recording UI was added.

## Live schema snapshot

`scripts/regenerate-api.sh` was run against `https://denchat.app/openapi.json` on
2026-09-15. `Packages/DenAPI/Contract/openapi.json` is byte-identical to what the server
served, 113,885 bytes, SHA-256
`f262e47901ca75bf685b57a60014ba942d7607a7d90a30c9dd18cbb0e4ee2b5f`, and
`Sources/DenAPI/openapi.json` is derived from it by `prepare-client-schema.py`, which
drops the 15 `additionalProperties: false` constraints and changes nothing else.

The canonical contract is one line, because that is what the server sends. Review it with
a semantic diff rather than by reading the file.

## Differences between the committed snapshot and the live schema

Nothing the native app calls changed. Every difference is an addition. The semantic diff
from the contract as committed in `b4a7b1c` to the one saved here: 18 paths added, none
removed, `/ws` the only changed path; 19 schemas added, none removed; 4 schemas changed,
each gaining one optional property; 3 `Event` tags added, none removed.

| Difference | Native effect |
| --- | --- |
| 18 new paths under `/rooms/{id}/music`, `/settings/sounds`, `/users/me/sounds`, `/users/me/voice`, `/sessions/{id}/recording`, `/users/me/hosts/{id}/recording` | None. No native caller; regeneration only adds unused client methods. |
| 19 new schemas for those features (`MusicQueue`, `SoundPack`, `VoicePreferences`, `CameraSettings`, `TerminalRecording`, …) | None. Not referenced. |
| `Bootstrap.display_name`, `Register.display_name` | None. The app has no bootstrap or registration screen. |
| `TerminalState.recording_enabled` | None, and optional. `TerminalState` is never decoded natively; Settings reads `Host` and `Grant` only. |
| `Event` gains `music_queue_updated`, `sounds_updated`, `voice_preferences_updated` | None. `API.Event` is never used; `AppStore.receive` routes on the wire tag and ignores tags it does not know. |
| `GET /ws` gains optional `music` and `sounds` query parameters | Deliberately unused. See below. |

No path, operation, parameter or response used by `DenService` was removed or changed,
so the currently generated client keeps compiling against the live schema.

## The gap that was real: `user_updated`

`crates/den-server/src/profiles.rs` broadcasts `Event::UserUpdated { user }` after a
profile patch, an avatar/banner change (`profile_images.rs`) and lazy status expiry
(`auth.rs`). `ws.rs::allowed` maps it to `None`, so it reaches **every** authenticated
socket and is **not** behind the music or sounds opt-in. `AppStore.receive` had no case
for it, so a rename only appeared after a refetch or a relogin.

`AppStore.apply(_:)` now upserts the user by id, replaces `user` when it is the signed-in
identity, refreshes call names and saves the cache. Every existing label reads `users` or
`user`, so message authors, DM titles, People, Settings, mentions and avatar initials all
follow without any new UI. It deliberately does not refetch: the event carries the whole
user, and a refresh would cost a round trip and risk disturbing loaded history. An unknown
id is appended rather than triggering the refresh that an unknown *channel* does.

`bio`, `accent`, `status` and the image URLs are kept current in the store even though no
native surface renders them yet. That is intentional: those surfaces are out of scope, and
storing the whole user means they need no second event path when they are built.

## Live call titles

`CallControllerContext.title` and `CallSession.title` were snapshots taken when a call was
reported, so a rename during a DM call stayed stale in the dock, the call sheet and the
CallKit caller name. `CallController.updateNames(_:title:)` now recomputes the title for
each live context from current names and reports a `CXCallUpdate` for an incoming call.
Outgoing calls are left alone: they were never reported with a `localizedCallerName`, and
giving them one now would be a UI change, not a parity fix.

`AppStore.refresh()` routes through the same path, so a reconnect corrects a title too.

Participant names had a second, narrower staleness. `CallControllerCredentials.names` is
captured when identity is restored, and `CallSession.join` assigns the names it is handed
before its first suspension point. A rename arriving between those two moments was applied
to the session and then overwritten by the older credential snapshot, for the length of the
call. The controller now keeps the latest snapshot and `connectMedia` resolves it at join
time through `CallController.joinNames(_:)`; the credential snapshot still covers a call
that starts before the controller has ever been told any names. `AppStore.clearSession()`
drops the snapshot so one account's names cannot label the next account's calls.

## WebSocket opt-ins

iOS has no music or sound consumer, so the socket URL stays ticket-only and the client
stays opted out rather than receiving events it would drop. `AppStore.socketURL(origin:
ticket:)` is the single construction site, used by `connectSocket()` and asserted directly
in tests. Unknown tags remain a `default: break`.

`sendTyping` emits `API.ClientEvent.case4`, a generator-assigned position, and that
position is not something to take on trust. All four `ClientEvent` variants are inline
rather than `$ref`s, so every generated payload type is named by its oneOf position, and
`object_open` and `object_close` are shape-identical apart from their `type` constant.
A regeneration that renumbers them could keep compiling. There is no stable generated type
to switch to, so the construction moved into `AppStore.typingFrame(channelId:)` and a test
pins the bytes it produces to `{"type":"typing","channel_id":...}`. `typing` is still the
fourth variant in the live schema; the test is what keeps that true rather than an
assumption. The receive path does not have this problem: it routes on the wire tag.

## Evidence

`DenTests/ProfileUpdatesTests.swift` covers the store, the socket URL, the emitted client
event, and the call wiring. The call tests use the real `CallController` with a stub
`CallControllerAPI` and contexts inserted directly. A `URLProtocol` that never answers the
call-token request holds `CallSession.join` suspended at `.connecting` with its callID,
title and names installed, so both the pre-join and mid-connect renames are proved against
a genuinely connecting call without a LiveKit room. `CallSession.names` is `private(set)`
rather than private so the test can read the map the join actually installed; that is the
only way to tell the fresh snapshot from the credential one, and reverting the `connectMedia`
call site alone now fails the test. Outgoing contexts are used throughout, so no CallKit
report is made and none is faked.

Not covered, and not claimed: the incoming CallKit system sheet. The `CXCallUpdate`
reported for an incoming call needs a call actually reported to CallKit, so what the caller
name looks like on that sheet is device acceptance, not a test result.

## Intentionally absent

No music queue, sound pack, voice preference or profile editor UI. No terminal recording
controls. No registration or bootstrap screen. No native outgoing screen broadcast. These
have live endpoints and no native surface, and this change does not add one.
