# M5 native server support

Scope follows the server addendum in [M5a brief](M5A-IOS-BRIEF.md).
`astra-ios` owns this server work in `/mnt/storage/den-server-ios`, branch
`server-ios`. The iMac's `den-ios` lane owns `apps/ios` and native/cloud tests.
The server change was deployed to production on September 14, 2026, after the
implementation merge and a separate deployment request. Evidence is below.

## Device registration and alert push

`POST /devices` accepts generated type `RegisterDevice`:

```json
{"platform":"ios","token":"<hex APNs token>","app_version":"1.0"}
```

It returns `Device { id, platform, app_version }` with status 200. It never
returns the token. Registration is idempotent by token and APNs environment;
re-registering updates the owning account, credential, app version, and generation.
`DELETE /devices/{id}` returns 204 for the owner, or 404 for a missing/other
user's registration. Call it before logout. Foreign keys also remove devices
when their bound login session or API token is deleted, including logout and
token revocation. Delivery ignores expired sessions.

The sender uses the same mention, DM, and followed-channel decision as the
WebSocket notification event. It sends HTTP/2 requests with an ES256 provider
JWT, topic `app.denchat.ios`, `apns-push-type: alert`, priority 10, channel
collapse ID, and an expiry. The payload contains the channel/message IDs,
notification reason, a bounded message preview, and the inbox notification count
as badge. It uses a bounded queue so a network request does not hold up chat.
Transport or transient Apple failures retain registrations and log no tokens.
Delivery is best effort; there is no persistent retry queue.

BadDeviceToken, DeviceTokenNotForTopic, and current Unregistered responses remove
the registration. An old response cannot delete a newly registered generation;
Unregistered timestamps older than the latest registration are also ignored.
Queued jobs recheck ownership, credential validity, channel membership, and
pending invitation status before sending. An already transmitted push cannot
be recalled during logout or account switching.

These rules follow Apple's [request](https://developer.apple.com/documentation/usernotifications/sending-notification-requests-to-apns)
and [token authentication](https://developer.apple.com/documentation/usernotifications/establishing-a-token-based-connection-to-apns)
interfaces. Tests use a local HTTP/2 stub and a disposable test-only EC key pair.
They never contact Apple.

Configuration to enable APNs through `/etc/den/den.env`:

```sh
DEN_APNS_KEY_PATH=/path/to/private/apns.p8
DEN_APNS_KEY_ID=<key ID>
DEN_APNS_TEAM_ID=<team ID>
DEN_APNS_ENV=sandbox
```

`DEN_APNS_ENV` is `sandbox` by default or `production` for distribution builds.
It selects both the Apple endpoint and the registration environment. Use a
server configured for the app's signing environment and register again after
changing that environment. The key, key ID, and team ID must all be supplied to
enable sending. If any are absent, startup logs one disabled message and chat
continues. Invalid supplied environment/key configuration produces a startup
error. The private key was still absent on Debian at implementation time, so
real APNs delivery and closed-phone notification taps remain unverified.

## DM invitations

`POST /calls/{channel_id}/invite` returns generated type `CallInvitation`:

```json
{"channel_id":"<DM ID>","from_user_id":"<caller ID>","expires_at":1789400045}
```

The caller must belong to the DM/group DM and already have a connection in
its call. Text/voice channels return 400, nonmembers receive 404, and a member
who has not joined receives 409. One invitation is active per channel for 45
seconds. The active caller's retries return the original invitation without
re-notifying; another caller receives 409 until expiry.

Only other current DM members with a pending invitation receive the socket event:

```json
{"type":"call_invite","channel_id":"<DM ID>","from_user_id":"<caller ID>","expires_at":1789400045}
```

`expires_at` is Unix seconds. Clients dismiss at that deadline. Server filtering
also suppresses expired or declined events. A conditional cleanup task removes
expired state without deleting a later invitation, and startup/hourly cleanup
removes expired rows left across restarts.

`POST /calls/{channel_id}/invite/decline` takes `DeclineCallInvitation`:

```json
{"from_user_id":"<caller ID>","expires_at":1789400045}
```

Copy these values from the invitation being dismissed. The endpoint declines
for that recipient account and returns 204, including retries of the same
decline. A missing, expired, replaced, or unauthorized invitation returns 404.
Matching the caller and expiry prevents a delayed decline from dismissing a
new call. Declining does not disconnect other participants.

When alert APNs is configured, an invitation uses the same custom fields as
the socket event, plus an `aps.alert` and sound, with its 45-second APNs expiry.
This is an ordinary notification to an alert token. Actual PushKit registration,
VoIP topic/delivery, CallKit accept/cancel signaling, and locked-phone ringing
remain M5b work. The web keeps its existing call join banner without ringing.

## Generated clients

`den-core` defines `DevicePlatform`, `RegisterDevice`, `Device`,
`CallInvitation`, `DeclineCallInvitation`, and `Event::CallInvite`.
`/openapi.json` derives these from the server; `apps/web/src/lib/schema.d.ts`
is generated with `openapi-typescript 7.13.0`.

Native generation also exposed existing schema errors. Message pagination and
message-search parameters now explicitly use `in: query`. Nullable `Id` fields
now emit inline `type: [string, null]`, preserving categories, reply references,
read markers, and host fields in Apple's generated Swift client. The runtime
JSON contract is unchanged. The iMac lane verified the old nullable-reference
form was dropped by Swift OpenAPI Generator 1.9 and 1.13.1.

An exact branch schema snapshot is supplied to the iMac at
`~/.local/share/den-ios-tools/server-ios-openapi.json`, with a matching Debian
artifact at `/mnt/storage/den-server-ios-openapi.json`. Production now serves
the same schema at `https://denchat.app/openapi.json`, verified by parsed JSON
equality. The compact HTTP response has different whitespace from the snapshot.
The final snapshot's SHA-256 is
`b2fd728d912bbcdae29020f28f501a2a628d6051ce47386614dcc1a1dfa5f480`.

## Verification and native fixture

All checks below passed:

- `cargo fmt --all -- --check` and Clippy across all workspace targets with
  warnings denied.
- `cargo test --workspace`: **40 passed**, including five new API tests and
  four HTTP/2 push tests.
- `cargo build --workspace` and SQLx offline metadata verification against all
  nine migrations using `cargo sqlx prepare --workspace --check -- --all-targets`.
- The M1 CLI smoke: independent sessions, WebSocket reconnection, an 11.9 MB
  41-second video upload with authenticated range playback, bot credentials,
  and exact fixture cleanup.
- Web schema regeneration, Svelte/TypeScript check with zero errors or warnings,
  and production build. The build reports its existing large-chunk advisory.
- Isolated fixture startup/seeding, private file modes, a single disabled-APNs
  startup log, health, process identity verification, shutdown and exact cleanup.

The iMac lane requested a disposable server for native UI tests. Its Debian
loopback port is **32827**; its private credential file is
`/mnt/storage/den-ios-fixture-07oluy_m/credentials.json`, mode 600 within a mode
700 directory. It contains two disposable users, their sessions, and the seeded
general/DM IDs. The fixture has its own database and uploads and does not use
friends' data. Native tests can reach it through an SSH local forward. It has
no LiveKit or APNs configured, so it proves text flows only.
It was started before the final decline-body guard; create a fresh fixture from
the merged binary before testing the final call-invitation API.

The native lane owns its remaining use and teardown. Stop it once that lane
has finished with it:

```sh
python3 /mnt/storage/den-server-ios/scripts/m5-ios-fixture.py stop \
  --dir /mnt/storage/den-ios-fixture-07oluy_m
```

The helper verifies the PID's start time and database environment before stopping
it, then deletes only its exact fixture directory. `DEN_SMOKE_KEEP=1` preserves
the files after stopping. A separate fixture start/seed/stop/cleanup smoke was
run without disturbing the native lane's server. Create a fresh fixture later
with `scripts/m5-ios-fixture.py start`; it prints only the port and private
credential-file path.

Builds use `CARGO_TARGET_DIR=/mnt/storage/den-m5-target`. Check logs are under
`/mnt/storage/den-m5-*.log`; no user credentials or production APNs keys belong
in those logs or in Git.

## Production deployment, September 14, 2026

Deployed `main` commit `7198f6290741a5494da33a89446efc0bd1a85832` using
`CARGO_TARGET_DIR=/mnt/storage/den-m5-target ./deploy/release.sh` from the main
checkout. First ran `ssh vps 'sudo systemctl start den-backup'` successfully.
The off-VPS archive is `/mnt/storage/den/backups/2026-09-14/den.zip`, with a
completion receipt at `2026-09-14T15:14:53Z`. The new server started at
`2026-09-14T15:17:45Z`; all nine migrations are recorded as successful.

Public health returned HTTP 200 and `{"ok":true,"version":"0.2.1"}`. The
installed server binary SHA-256 matches the local release binary:
`e3cd388d5373163599509959c31fb3c428c0ba11bd585a03f5af5f0c184aa2b7`.

Public OpenAPI exposes `POST /devices`, `DELETE /devices/{id}`,
`POST /calls/{channel_id}/invite`, and
`POST /calls/{channel_id}/invite/decline`. The query parameter locations and
inline nullable IDs match the exact schema already supplied to the native lane.
Unauthenticated device/invitation POST requests returned 401. The compact public
schema SHA-256 is
`50ab656df983189a10c7823572c7a8c6891ebe31cc85da79001a313f4f9ad0cf`.

Service invocation `02cf744cb8b94581a18f76e7180ee4a6` logged exactly one
`APNs is not configured; push delivery disabled` line and no startup errors.
APNs delivery still needs the missing credentials; the endpoints remain usable.

Both public smokes passed against `https://denchat.app`:

- M1 public adaptation at `/mnt/storage/den-m5-public-m1.py`: existing test users
  log in through independent CLI sessions, send/read/tail messages, and reconnect
  the same tail processes after a loopback relay cuts only their public WebSocket
  connections. Production is not restarted for this test. A 41-second H.264/AAC
  clip, 11,894,863 bytes, passes chunk upload/resume, authenticated HEAD/range,
  and full HTTP decoding. A new token for the existing bot sends successfully;
  revocation returns 401 and a replacement works. Cleanup verifies removal of
  five exact messages, one upload, new tokens, and three login sessions.
- `scripts/m3-smoke.mjs`: public media at `135.148.120.197`, three cameras,
  screen/audio delivery, mute/leave, push-to-talk and custom key/blur handling,
  text during calls, DM privacy and switching, resync, and desktop/mobile
  screenshots. Cleanup removed its one message and verified no new room
  artifacts. Desktop screen-share and mobile grid screenshots were inspected.

Local evidence: `/mnt/storage/den-m5-production-{release,m1,m3,startup}.log`,
`/mnt/storage/den-m5-production-openapi.json`, and
`/mnt/storage/den-m5-production-shots/`. Public M1 cleanup receipts are in
`/mnt/storage/den-public-m1-67pmui9e/cleanup-receipts.json`.
