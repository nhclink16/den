# M3 voice and video

Implemented the server and browser work in `M3-BRIEF.md`. The demo is running at
https://codexbox.tail44c455.ts.net:4200. LiveKit `v1.9.0` runs in Docker with host
networking; signaling uses `wss://codexbox.tail44c455.ts.net:4202`. Media candidates
are restricted to `100.116.27.23`, UDP 50000–50200, with TCP 7881 available.
`den-demo` has been rebuilt and restarted. Its database was backed up before the
migration; all previous row counts, foreign keys, and integrity checks passed.

## Run

The original `/tmp/claude-1000/.../scratchpad/dev` folder was absent. A replacement
was seeded at `~/.local/share/den-dev` with `nicholas`, `bob`, and `ari`. Their shared
dev password is in the private `credentials.json` there. The demo's existing
accounts and passwords were preserved.

```bash
# Terminal 1, from any directory
~/.local/share/den-dev/run-server.sh

# Terminal 2, from the repo root
npm --prefix apps/web ci
npm --prefix apps/web run dev

# Terminal 3, from the repo root
npm ci
node scripts/m3-smoke.mjs
```

The smoke script defaults to `http://localhost:5173` and reads that private
credentials file. Override with `DEN_SMOKE_URL`, `DEN_SMOKE_PASSWORD`, or
`DEN_SMOKE_CREDENTIALS`. It needs `/usr/bin/chromium`; `playwright-core` is pinned
in the root package. Run this against seeded test data: it sends a text message
and opens a DM. It does not print credentials.

See [deploy/README.md](../deploy/README.md) for the exact Docker command,
environment variables, config rendering, Tailscale routes, and demo restart.
For schema updates, run the API and use:

```bash
cd apps/web
npx openapi-typescript http://127.0.0.1:7000/openapi.json -o src/lib/schema.d.ts
```

## Contract

| Endpoint | Behavior |
| --- | --- |
| `POST /calls/{channel_id}/token` | Returns `{ url, token }`. Authenticated members can join text and voice channels; DMs require explicit membership. Inaccessible or missing channels return 404. Missing LiveKit configuration returns 503 with `voice_unavailable`. |
| `GET /calls` | Returns `CallState[]`, one row per visible channel, including empty participant lists. It reconciles with LiveKit to recover missed webhooks and Den restarts. A media service outage retains the last snapshot so text can still boot. |
| `POST /livekit/webhook` | Accepts the raw LiveKit webhook body and its JWT `Authorization` signature. Uses the official Rust SDK to verify the signature and body hash. Missing, malformed, or tampered signatures return 401; valid events return 204. This endpoint uses LiveKit authentication, not a Den cookie or bearer token. |

`CallToken` and `CallState` live in `den-core`; the browser types were generated
from `/openapi.json`. `Event::CallState { channel_id, participant_ids }` is emitted
for participant joins, leaves, and room completion, with the existing WebSocket
visibility filtering. Resync now includes `/calls`. Tokens expire after ten
minutes, identify the Den user, carry the display name, and grant publishing and
subscription only in the requested channel's room. No room-admin grant is issued.

Migration `0003_voice.sql` extends the channel-kind constraint and seeds one
uncategorized `hangout` at position 0. Each database gets its own room ID. Voice
rooms reject message creation at the API and database levels. The startup
migrator preserves child rows while rebuilding the channel table; run migrations
through `den-server`, as described in the deployment README. Applied migrations
0001 and 0002 were not changed.

## Browser behavior

`call.svelte.ts` owns the LiveKit room, participant snapshots, device preferences,
remote audio elements, and call controls. Sidebar voice rows join immediately;
clicking the current room does nothing and another room switches calls. The dock
stays visible when the sidebar is collapsed or on mobile. Text navigation leaves
the call connected. Logout releases tracks and disconnects.

The 160px strip keeps chat usable; expansion replaces chat and the composer with
a grid. Screen shares come first. In the strip, the 384px-wide share is fitted to
the available height so its name label remains visible. Local camera video is
mirrored. Active-speaker rings use LiveKit's speaking state. DM headers have a
Call button and other participants see the join banner without ringing or push.

Voice settings include input mode, a captured PTT key, microphone/camera/speaker
selection, a live mic meter, and saved camera/sound preferences. Speaker selection
only appears where supported. The preview microphone and analyser stop when the
settings section closes. PTT releases on keyup, blur, visibility changes, and
pointer cancellation. M/V/S shortcuts ignore editable fields and modifiers.
Join/leave sounds use two quiet sine notes; mic processing, adaptive stream, and
dynacast are enabled.

The browser SDK is pinned to `livekit-client 2.15.6`. Version 2.22.3 established
connections to server 1.9.0 but timed out negotiating track publication in the
smoke test. Version 2.15.6 passed the same test. The [SDK documentation](https://docs.livekit.io/reference/client-sdk-js/)
provides the room, track attachment, and device-selection APIs used here.

## Verification

Passed on 2026-09-12:

- All 14 server integration tests and workspace tests. The two new tests cover
  token authorization, DM exclusion, voice-message rejection, unconfigured 503,
  and webhook signature/body rejection plus a valid event.
- Formatting, Clippy with warnings denied, and SQLx metadata verification.
- Svelte/TypeScript checks with zero errors or warnings, and production build.
  Vite reports the expected bundle-size advisory for the bundled media SDK.
- `scripts/m3-smoke.mjs` with the required fake-media Chromium flags. It checks
  two-user tile membership, remote mute within two seconds, leave cleanup,
  three decoded cameras, decoded screen sharing, received audio packets and
  playing audio elements, PTT/custom key/blur, the mic meter, typing and shortcuts,
  DM privacy and switching, reload/resync, and saved camera state.
- The call and UI smoke flows against the built SPA over temporary tailnet HTTPS on 4203 with
  WSS signaling on 4202. WebRTC stats confirmed `100.116.27.23` as the selected
  media peer. The temporary route was removed and localhost dev restored.
- Demo HTTPS health and LiveKit signaling probes, live process checks, and the
  demo migration's before/after database counts and integrity checks.

Screenshots in [shots](shots/) show the strip, grid, and dock at 1440×900 and
390×844, plus screen sharing, DM calling, and Voice settings. They were opened
and inspected. The green test video comes from Chromium's fake camera; no new UI
colors were added. These checks used three isolated Chromium contexts on this
box with fake media. Physical-device audio quality, Bluetooth, and an actual
three-person gaming session were not tested here; the separate iOS lane remains
outside M3.

Stopped after the notes and passing smoke.
