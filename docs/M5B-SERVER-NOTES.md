# M5b server calls

Implemented and verified on `server-ios-calls` in
`/mnt/storage/den-server-ios`. Native owns `apps/ios`; this lane owns the
server, shared types, migrations, and generated OpenAPI/web schema.

## Contract agreed with den-ios

`POST /devices` adds purpose `alert | voip`, defaulting to alert, environment
`sandbox | production`, defaulting to `DEN_APNS_ENV`, and an installation UUID
`client_id`, required for VoIP and optional for older alert clients. Token
uniqueness includes purpose and environment. The receipt adds these fields.
Credentials and registration generations still control ownership and logout.

`CallInvitation` adds an `id` ULID. New actions must identify this generation.
`CallInvitationState` contains `invitation`, state `ringing | active | cancelled
| expired | ended`, accepted `{user_id, answer_id}` entries, and
`declined_user_ids`. Terminal state remains readable for ten minutes.

- `GET /calls/invitations` and `GET /calls/invitations/{invitation_id}` reconcile
  invitations visible to the authenticated DM caller or recipient.
- `POST /calls/{channel_id}/invite/accept` takes `{invitation_id, answer_id}`.
  The answer UUID and authenticated credential win atomically per recipient
  account. A matching retry returns 200 state; another device returns 409
  `answered_elsewhere`. Group recipients answer independently. The client then
  obtains media credentials through the existing `/calls/{channel_id}/token`.
- Legacy decline keeps `{from_user_id, expires_at}` and accepts optional
  `invitation_id`. It cannot undo an accepted answer.
- Caller-only `/invite/cancel` and `/invite/end` take `{invitation_id}`.
  Cancellation is allowed before acceptance. End closes invitation signaling,
  without disconnecting LiveKit participants. Room/caller departure also
  reconciles signaling, while another connection from that account preserves it.
- `POST /calls/invitations/redeem` takes `{ticket}`. This narrowly scoped read
  capability requires no Den bearer, returns only invitation state, and is bound
  to the destination registration generation and credential. Invalid, expired,
  or replayed tickets return 410.

The canonical event is `{type:"call_invitation_state", call:CallInvitationState}`.
Legacy `call_invite` adds `invitation_id`. Requested named events
`call_invite_accepted`, `call_invite_cancelled`, `call_invite_expired`, and
`call_ended` also carry `call`; acceptance adds `user_id` and `answer_id`.
Native can consume the canonical state event alone.

The 45-second deadline stops unanswered ringing. It does not end accepted media.
No new endpoint imposes an account-level limit on media connections.

An accepted answer has a 20-second join grace. A complete LiveKit participant
snapshot must confirm that this recipient account has no connection before an
unjoined answer is cleared and marked declined. A provider outage preserves the
answer. Any verified connection from the recipient account satisfies the grace.
Other accepted recipients remain active. If none remain, the invitation returns
to ringing until its original deadline, then expires. The server checks media
roughly every five seconds; clients do not issue recipient abort or global end
when their own token fetch/join fails.

Invitation state and answer ownership survive restart. Timers resume expiration,
and authenticated reads refresh media. An old room-finished callback after
restart must first be checked against LiveKit. A matching terminal cancellation
retries with 204; an active or differently terminal cancellation returns 409.
Caller end retries with 204 for any retained terminal state. Unknown, invisible,
or purged IDs return 404; accepting a retained terminal ID returns 409. Legacy
decline returns 404 for stale/ambiguous caller-expiry tuples and cannot affect a
replacement generation. Other DM members receive 403 for caller-only actions.
All actions recheck authorization before mutation.

## PushKit transport

VoIP payload: `{type:"call_invite", invitation_id, channel_id, from_user_id,
from_display_name, expires_at, fetch_ticket, aps:{}}`. No LiveKit token or Den
session appears in it. Native reports to CallKit immediately from these fields,
before redeeming the ticket. Duplicate delivery can reconcile by authenticated
invitation GET.

VoIP uses `apns-push-type: voip`, topic `app.denchat.ios.voip`, priority 10, and
expiration 0. Alert and VoIP destinations route to their own registered
environment. An installation with a valid VoIP registration in the same
environment receives no duplicate alert invitation, while ordinary message alerts
continue normally. Legacy alert registrations without client_id cannot be paired
with an installation and retain alert delivery.

Apple requires initial VoIP wakeups to be reported through CallKit and directs
later cancellation/details over the established connection. The server therefore
sends VoIP only for incoming calls. See Apple's
[PushKit response guidance](https://developer.apple.com/documentation/pushkit/responding-to-voip-notifications-from-pushkit)
and [APNs request headers](https://developer.apple.com/documentation/usernotifications/sending-notification-requests-to-apns).

## Verification

- `cargo test --workspace`: 48 tests passed, including 37 server integration
  tests and 8 server unit/provider tests. Coverage includes simultaneous
  account-device answers, independent group answers, accept/cancel races,
  stale-generation actions, denied access, restart and real 45-second expiration,
  20-second abandoned join grace with provider outage, sibling connections,
  stale room callbacks, one-use ticket races/replay/expiry/rotation/logout, and
  actual HTTP/2 APNs headers, environment routes, and duplicate-alert suppression.
- `cargo fmt --all -- --check`, workspace clippy with warnings denied, and
  `cargo sqlx prepare --workspace --check -- --all-targets` against a fresh
  migrated disposable DB passed. No offline metadata changes were needed.
- Actual-binary OpenAPI regenerated the web schema with openapi-typescript
  7.13.0. Web check reports zero errors/warnings; production build passes
  (existing large-chunk advisory remains).
- `scripts/m5-calls-smoke.mjs` passed against isolated LiveKit 1.9.0: two peers
  publish and receive audio and decoded camera frames over the Tailnet; accept
  retries, recipient reconnection, caller sibling-device preservation, and
  signaling-only end all work. Every owned media connection closed afterward.

Final schema: `/mnt/storage/den-server-ios-calls-openapi.json`, also delivered to
`~/.local/share/den-ios-tools/server-ios-calls-openapi.json` on the iMac.
SHA-256: `c7d2f818ec3bf4f54d11dfd1008a5512d2717eceb95fd992f6930bc4a1b94d84`.
It includes `IncomingVoipCall` without provider-only aps, and the agreed generated
device/state/action types. The earlier provisional hash was b8eefe…28c3b;
only the accept endpoint description changed afterward to document join grace.

Native fixture handoff: Debian Den loopback port 45621, LiveKit loopback port
52695, private credentials `/mnt/storage/den-ios-fixture-esglonk1/credentials.json`.
Forward both ports unchanged to Mac localhost. Media UDP is bound only to the
Debian Tailnet IP. The exact fixture owns its database, uploads, accounts, and
labeled Docker container; existing friend data and the earlier native text
fixture are untouched. Native owns fixture use and final cleanup:
`python3 scripts/m5-ios-fixture.py stop --dir /mnt/storage/den-ios-fixture-esglonk1`.
`DEN_SMOKE_KEEP=1` stops the exact processes/container while retaining evidence.

Local logs: `/mnt/storage/den-m5b-workspace-tests.log`,
`den-m5b-clippy-final.log`, `den-m5b-sqlx-check.log`, `den-m5b-web-check.log`,
`den-m5b-web-build.log`, and `den-m5b-media-fixture-final.log` under the same root.
The stale-room restart test was observed failing before the reconciliation fix
and passing afterward (`den-m5b-restart-race-{red,green}.log`).

## Production

Deployed pushed main `e596399` on 2026-09-14; the server implementation is
`5f0d2b6`, merged through `5303d3a`. Fable was notified before the shared
contract/migration merge and coordinated the other lane's deployment hold.
Native received and verified the final schema before deployment.

The VPS `den-backup` service succeeded at 16:25:49 UTC (ExecMainStatus 0).
Off-box receipt: `/mnt/storage/den/backups/2026-09-14/den.zip`, 16,878,810 bytes,
with `complete` updated at 16:25:48 UTC. Then `deploy/release.sh` built and
installed this exact pushed main revision from `/mnt/storage/den-m5b-release`,
an isolated clean checkout. This kept the shared main checkout's unfinished web
dictation work out of the release and preserved all other lane files.

Public health: `https://denchat.app/health` returned
`{"ok":true,"version":"0.2.1"}`. Public OpenAPI matches the final artifact exactly
as parsed JSON, including devices, invitation accept/cancel/end/list/get/redeem,
and generated types. Its compact HTTP body SHA-256 is
`922859d7d43c36b16f6b81c0e129c805ff07c40256eeb9426011c5e1e3d235e7`.
Authenticated reads reject missing credentials with 401; the bounded public
redeem rejects an invalid ticket with 410.

Service invocation `b65d6644deb24f2aa46e5170673b511a` logged exactly one
`APNs is not configured; push delivery disabled` line and zero startup errors.
Real Apple delivery remains blocked on the missing APNs key/IDs; the HTTP/2
provider tests verify transport behavior without calling Apple.

Public M1 passed against `https://denchat.app`: independent CLI sessions and
messages, two reconnecting WebSocket tails after cutting only their own relay
connections, 41-second H.264/AAC video (11,894,863 bytes) with chunk resume and
full authenticated decode, and bot bearer revocation/replacement. Cleanup
verified five exact messages, one upload, new tokens and three login sessions
removed. Receipts: `/mnt/storage/den-public-m1-6trbzg08/cleanup-receipts.json`.

Public M3 passed: three cameras, decoded screen/audio, mute/leave,
push-to-talk/custom key/blur, live text, DM privacy/switching, resync, expected
public media IP 135.148.120.197, and desktop/mobile captures. Exact cleanup
removed its one test message and confirmed no new room artifacts.

The first mobile grid screenshot captured the portal layout before its content
moved into place. Repeating the same smoke with a 600 ms pause before screenshots
produced a correct three-video mobile grid; the full repeat and exact cleanup
passed again. Desktop screen-share and settled mobile grid images were visually
inspected. Extra evidence: `den-m5b-production-m3-settled.log` and
`den-m5b-production-settled-shots/` under `/mnt/storage`.

Both lanes were notified with the final production contract and schema. The
server release is complete; native owns its remaining device/CallKit verification
and fixture cleanup. Apple delivery still needs the APNs key/IDs.

Release evidence under `/mnt/storage/`: `den-m5b-production-release.log`,
`den-m5b-public-verification.json`, `den-m5b-production-openapi.json`,
`den-m5b-production-startup.log`, `den-m5b-production-m1.log`,
`den-m5b-production-m3.log`, and `den-m5b-production-shots/`.

For an attended persistent media peer, run from the server worktree or main:

```sh
DEN_IOS_FIXTURE_CREDENTIALS=/mnt/storage/den-ios-fixture-esglonk1/credentials.json \
  node scripts/m5-call-peer.mjs --user ios_blair --screen
```

This publishes synthetic microphone/camera and Chromium screen capture in the
fixture DM. Use `ios_alex` to test a sibling connection for that account; omit
`--screen` for camera/audio only. Ctrl-C closes only this peer. A 30-minute bound
also closes it automatically. Real start, publication and Ctrl-C teardown passed.
