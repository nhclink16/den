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

Backup, deployment and public M1/M3 smokes are the remaining authorized release
steps. APNs transport is verified against a local protocol stub; real Apple
delivery remains blocked on the missing APNs key/IDs. No production credentials
have been invented or added.
