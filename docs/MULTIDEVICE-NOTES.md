# Multiple devices in one call

Deployed to https://den.nicholascaron.com on 2026-09-13. One account can stay in a call on several devices at once.
Each connection can publish its own camera and screen share. Den does not impose
a per-account device limit. Available bandwidth and server resources still apply.

Join on the desktop first, then open Den on another device with the same account
and join the same room. Additional devices begin with their microphone and call
sound off, so the desktop headset keeps handling the conversation. Use the mic
and speaker buttons to change either setting on that device. Leaving or reloading
one device leaves the others connected.

Other connections from your account never play your own microphone back to you.
Their screen-share audio does play on devices with sound enabled. This lets a
secondary device supply a stream while the desktop supplies the headset audio.

The call counts each person once. Camera and screen tiles remain separate per
connection and show device labels when an account has several connections.
Several screen shares use separate grid cells rather than overlapping or each
claiming a full row. The layout scrolls when the viewport cannot fit every tile.

## Implementation

- Each token request mints a new LiveKit identity, `user_id:connection_id`.
  The suffix is a server-generated ULID. LiveKit reconnects retain the identity;
  a new join gets a new one.
- The existing call-state map keeps each connection and its LiveKit SID.
  `/calls` and WebSocket call events project those connections back to distinct
  account IDs. A late leave from an old SID does not delete a replacement.
- Existing account-only identities remain understood during rolling deployment.
  Refresh browser tabs after deployment to load the new device controls and
  account-aware audio rules.
- Clients subscribe to video independently of audio. Sound-off devices do not
  subscribe to remote audio, and own-account microphone tracks are never
  subscribed. This avoids relying on an HTML mute flag that the SDK may clear
  when resuming audio playback.
- No database migration or shared API shape changed. The server and UI continue
  using `CallToken` and `CallState` from the existing shared contract.

## iPhone screen sharing

This change allows simultaneous phone and desktop call connections. It does not
add native iPhone screen broadcasting. Sharing another iPhone app, including its
audio while Den is in the background, needs the platform's capture support.
LiveKit's supported native path is its [ReplayKit broadcast extension](https://github.com/livekit/client-sdk-swift/blob/main/Docs/ios-screen-sharing.md).
There is no native iOS target or broadcast extension in this checkout yet.

The browser tests below prove Den's handling of multiple media publishers, not
Instagram capture on a physical iPhone. Cross-app capture, captured app audio,
and iOS background behavior remain unverified on a physical device. The native
ReplayKit extension is the documented implementation path if the browser does
not provide the required cross-app capture.

## Verification

Build the server and client, then run:

```bash
cargo build --release -p den-server -p den
npm --prefix apps/web run build
scripts/run-multidevice-smoke.sh
```

The runner creates an isolated localhost Den instance, fresh SQLite database,
throwaway accounts, and a separate pinned LiveKit 1.9.0 container. It uses ports
17400, 17880, 17881, and UDP 50600-50800. It stops both services afterward and
prints the private scratch directory containing logs and screenshots. It never
joins the public hangout or touches the existing demo/dev data.

Passed:

- All 15 server integration tests, workspace tests, Clippy with warnings denied,
  formatting, Svelte/TypeScript checks, and production build.
- Token uniqueness, single-user presence across three connections, stale-SID
  leave protection, final-device departure, and DM membership enforcement.
- Four real WebRTC browser connections: three as Nicholas and one as Bob.
  Three simultaneous same-account cameras and screen shares decode correctly.
- Two secondary-device screen-audio streams arrive at the desktop. Unmuting a
  second own-account microphone adds no echo playback. Extra devices initially
  receive no call audio; their speaker control enables and disables it independently.
- Reloading and rejoining one device preserves the other connections. Leaving
  devices one by one keeps the person present until the final connection leaves.
- Desktop and 390px-wide device screenshots were opened and inspected. The
  expanded desktop grid fits three screens and four participant tiles without
  overlap. See [desktop](shots/multidevice-desktop.png) and
  [phone layout](shots/multidevice-phone.png).
- The original M3 browser smoke passes after the multi-device test, covering
  ordinary calls, mute/PTT, screen/audio delivery, DM switching, and resync.

Chromium uses fake cameras and fake full-screen capture. The test adds a generated
audio tone to each captured screen stream because headless Linux's fake desktop
capture supplies no system audio. This exercises actual LiveKit screen-audio
publication, transport, subscription, and playback. It is not a claim about
capturing audio from a particular phone app.

## Live deployment check

After a fresh backup, `deploy/release.sh` installed the tested server and web
client. Public health passed and the VPS server binary hash matched codexbox.
A private DM between the existing `m6_bob` and `m6_ari` test accounts then passed
three browser connections, including two on the same account, two decoded
cameras and screen shares, a quiet secondary connection, deduplicated presence,
and independent leave. WebRTC selected public peer `135.148.120.197`.
The test never joined public hangout, and all its browsers were closed afterward.
The live-check log is `/tmp/den-multidevice-public.log` on codexbox.
