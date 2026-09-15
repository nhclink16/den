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
of which `Bootstrap`, `Register` and `TerminalState` each gain one optional property while
`Event` gains three variants; no schema's `required` set changed in either direction.

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
**every** live context, incoming and outgoing alike, so `CallControllerContext.title` and
the `CallSession.title` the dock and call sheet read are refreshed in both directions.

What differs between the two is only the report to CallKit. An incoming call was reported
with a `localizedCallerName`, so it gets a corrected `CXCallUpdate`. An outgoing call never
had one — CallKit shows its generic handle — and inventing one now would be new UI rather
than a parity fix, so that reporting is deliberately unchanged. An outgoing call's native
title is refreshed; only what CallKit displays for it is left as it was.

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

`sendTyping` emits `API.ClientEvent.case4`, a generator-assigned position. All four
`ClientEvent` variants are inline rather than `$ref`s, so every generated payload type is
named by its oneOf position and there is no stable generated type to name instead.

That position is weaker than a name, but it is not unguarded today. The call site passes a
`channelId:` label and a `.typing` case, and both are specific to the typing payload's own
generated types, so a reorder that moved another variant into position four would generally
fail to compile rather than silently emit the wrong event. This is not a claim that the
current call could quietly become `object_open`.

The construction still moved into `AppStore.typingFrame(channelId:)`, with a test pinning
the bytes it produces to `{"type":"typing","channel_id":...}`. That guards what compilation
does not: a future regeneration, a naming-strategy or generator change, or a later hand
adaptation of this call that still type-checks but emits something else. The receive path
has no equivalent exposure, because it routes on the wire tag.

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

### Results

Run on the iOS 27 simulator. The build plugin was confirmed to generate from the schema
saved here, producing the `MusicQueue` and `SoundPack` types, so the regenerated contract
is proved to compile the existing native client rather than only to parse.

Every one of the ten tests has both a green run and a red one under a deliberate mutation,
so none of them is passing vacuously. Each mutation was restored byte-exact afterwards.

| Run | Result | Log |
| --- | --- | --- |
| Unmutated | 10 passed, 0 failed, 0 skipped | `/tmp/den-ios-parity-first-green.log` |
| Store, socket URL and typing mutations | 7 failed, 0 passed, 0 skipped | `/tmp/den-ios-parity-store-negative.log` |
| Call wiring mutations | 3 failed, 0 passed, 0 skipped | `/tmp/den-ios-parity-calls-negative.log` |
| Restored full `DenTests` plus both existing text UI tests | 57 passed, 0 failed, 1 skipped | `/tmp/den-ios-parity-full-regression.log` |

The seven and the three are disjoint and cover all ten. The three call mutations each failed
on the assertion they were aimed at: reverting `connectMedia` to the credential snapshot read
`Ann` instead of `Annabel` and left `user-bo` absent; dropping the `isLive` gate relabelled an
ended context; and removing the `clearSession` reset kept the previous account's names and
defeated the cold-answer fallback.

The connecting-call test reached a genuine `.connecting` state rather than timing out, so its
result is about the rename rather than about the audio stack.

The restored run includes 55 passing `DenTests` and two passing `DenUITests` against the
isolated current-main loopback server: `testNativeTextFlowAndSessionRestoration` and
`testSearchResultRestoresUsableTabNavigation`. They exercise real login, Keychain restoration,
sending with Return, editing, replies, reactions, deletion, DMs, unread mentions, search,
light/dark appearance updates and sign-out. The single skip is the opt-in
`platformPermissionCallbacksResumeOnOwningActor` system-permission integration test, not
any compatibility or profile test. No existing assertion was weakened.

XCTest screenshots of the conversation, appearance and restored search navigation were
exported and visually inspected in `/tmp/den-ios-parity-ui-evidence`. These do not show the
incompatible-response Update Den panel; that panel's appearance remains visually unverified.
The separate simulator accessibility bridge timed out, but the Xcode UI-test runner worked.
The incoming CallKit system sheet is still unverified as described above; this run did not
join a real media room or install a physical-device build.

The result bundle is
`~/Library/Developer/XcodeBuildMCP/workspaces/den-65f821c5653d/result-bundles/test_sim_2026-09-15T19-29-53-922Z_pid20117_f51de558.xcresult`.
The current live OpenAPI was fetched again and remained byte-identical to the saved raw
contract. The parent also ran `ci_post_clone.sh` locally: project, package pins, derived
schema and refreshed fixture archive checks passed. This is not an Xcode Cloud run.

No known contract mismatch remains in the existing native API calls examined here.
Unknown event tags are ignored; invalid or missing required known response data still
triggers the deliberate Update Den state from `b4a7b1c`. Already-installed builds retain
their old decoding behavior. No TestFlight upload was attempted, no certificate was added,
and Andy's enrollment and the availability of build 0.3.0(1) were left unchanged.

## Intentionally absent

No music queue, sound pack, voice preference or profile editor UI. No terminal recording
controls. No registration or bootstrap screen. No native outgoing screen broadcast. These
have live endpoints and no native surface, and this change does not add one.
