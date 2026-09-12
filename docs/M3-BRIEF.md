# M3 brief: voice and video

Owner: Astra (voice lane). Fable wrote this and reviews the result. Read `docs/DESIGN.md`, `docs/ROADMAP.md`, `AGENTS.md`, `docs/M2-NOTES.md`, then `apps/web/src/app.css` and the existing components before writing UI. Match the existing visual language exactly: the room is dim, only what is live is lit amber. No green, no blue, no new colors.

Done when: three people can sit in the hangout with cams on from browsers on the tailnet, screen share works, push-to-talk works, and the text chat stays usable during the call.

## Server

- Run LiveKit self-hosted on this box for dev. Docker is installed without the compose plugin, so use `docker run` with the pinned image `livekit/livekit-server:v1.9.0`, host networking, config from `deploy/livekit.yaml`. Dev API key and secret come from env. Put the exact run command in `deploy/README.md`. The tailnet demo instance is a user systemd service `den-demo` on port 7200 behind `tailscale serve` on 4200; add a second serve route for LiveKit signaling and make media work over the tailnet (node IP `100.116.27.23`, UDP port range from the yaml). Restart `den-demo` after changes so Nicholas can test at `https://codexbox.tail44c455.ts.net:4200`.
- `POST /calls/{channel_id}/token` → `{ url, token }`. Only members of the channel (any member for text/voice channels, DM participants for DMs). Room name is the channel id. Token identity is the user id, name is the display name. Config via `DEN_LIVEKIT_URL`, `DEN_LIVEKIT_API_KEY`, `DEN_LIVEKIT_API_SECRET`. Refuse cleanly with a 503 and a clear message when LiveKit is not configured.
- Call state: track who is in which room via LiveKit webhooks (`participant_joined`, `participant_left`, `room_finished`) on `POST /livekit/webhook`, verified with the LiveKit webhook signature. Broadcast `Event::CallState { channel_id, participant_ids }` to everyone who can see the channel. `GET /calls` returns the current state for all visible channels. Include it in the resync contract.
- Seed one voice channel named `hangout` with `kind = voice`, uncategorized, position 0, in the migration that adds anything else you need. Voice channels are joinable rooms, not text rooms: messages in them are not allowed.
- Add the new types to `den-core`, regenerate nothing by hand; the client regenerates `schema.d.ts` from `/openapi.json` (`npx openapi-typescript`).
- Tests: one integration test for token minting authorization (member ok, outsider 404, DM outsider 404, unconfigured 503) and one for webhook signature rejection. That is enough.

## Client (Svelte 5, apps/web)

Use the `livekit-client` npm package, pinned. Do not use LiveKit's React components. All call state lives in a new `src/lib/call.svelte.ts` store next to `store.svelte.ts`, with the same style: a class with runes, exported singleton.

### Joining and leaving
- Voice channels render in the sidebar under their category with a headset icon (add `headset` to `Icon.svelte`, 16px line icon, 1.5 stroke). Under the room name, when people are in it, show their avatars at 20px overlapping by 6px, max 5 then `+n`. Clicking a voice room joins it immediately. Clicking another voice room switches. Clicking the current one does nothing.
- Join with mic on and camera off by default. Remember the last mic and camera state in `localStorage`. Play a short soft tone on join and leave generated with WebAudio (two sine notes, 80 ms each, under -20 dB). No audio assets.
- Constraints: `echoCancellation`, `noiseSuppression`, `autoGainControl` all true. Adaptive stream and dynacast on.

### The call dock (always visible while in a call)
In the sidebar, directly above the user row at the bottom, a 44px tall bar with `background: var(--bg-3)` and a top hairline `var(--line)`:
- Left: a 7px amber dot with a soft glow (`box-shadow: 0 0 8px var(--lamp)`), then the room name in 13px bold, then the participant count in mono 11px `var(--ink-2)`.
- Right: four icon buttons, 28px square, 6px radius: mic, camera, screen share, leave. Active mic and camera show `var(--ink)`, muted shows `var(--ink-3)` with a diagonal slash variant of the icon. Screen share active shows amber. Leave is `var(--ember)` on hover only, otherwise `var(--ink-2)`.
- In push-to-talk mode, replace the mic button with a pill that reads `hold \` to talk` in mono 11px, and lights amber while the key is held.

### The docked strip (in a call, viewing any text channel)
Between the channel header and the message list, a horizontal strip 160px tall, `background: var(--bg-2)`, bottom hairline, 12px padding, 10px gap, scrolls horizontally when it overflows, no visible scrollbar.
- Tiles are 16:9, 216px wide, `border-radius: var(--r-lg)`, `background: var(--bg-3)`, `overflow: hidden`. Video fills with `object-fit: cover`. Local camera is mirrored.
- Camera off: the user's `Avatar` at 44px centered.
- Name label bottom-left: 12px, `var(--ink)`, on a pill `rgba(0,0,0,.45)` with 4px 8px padding, 6px from the edges. Muted mic: a 14px slashed mic icon inside the same pill, after the name.
- Speaking: a 2px ring `var(--lamp)` via `box-shadow: 0 0 0 2px var(--lamp), 0 0 12px var(--lamp-glow)`, transition 120 ms. Use LiveKit's `isSpeaking` on the participant.
- Screen share tile: 384px wide, placed first, with a small `screen` icon in the pill.
- Top-right of the strip, a 28px `expand` icon button that toggles the full grid.

### Full grid (expanded)
Replaces the message list and composer in the main column. Header stays. `display: grid; grid-template-columns: repeat(auto-fit, minmax(280px, 1fr)); gap: 12px; padding: 16px;` tiles as above but fluid. A screen share spans all columns and sits on top. Bottom center, floating 12px above the bottom edge: a toolbar with the same four buttons at 40px plus the PTT pill, `background: var(--bg-2)`, hairline border, 999px radius, 8px padding, soft shadow. A `collapse` button top-right returns to the strip. The member drawer stays where it is.

### Keys
`M` toggles mic, `V` camera, `S` screen share, when focus is not in an input. PTT key default is Backquote, configurable in settings. PTT works while the page has focus; the global hotkey comes with the Tauri shell in M4, do not attempt it here.

### Settings → Voice (new section, between Notifications and Layout)
- Input mode: radio, `Voice activity` (default) or `Push to talk`, with a key capture field for the PTT key.
- Microphone, Camera, Speaker: `<select class="field">` populated from `enumerateDevices()`; ask for permission on first open with a `btn lit` labeled `Allow microphone and camera`. Speaker select only where `setSinkId` exists.
- A mic level meter: a 6px tall bar, `var(--line)` track, amber fill driven by an AnalyserNode, updating at 30 fps while the section is open.
- Two switches: `Join with camera on`, `Play join and leave sounds`.

### DM calls
In a DM channel header, a `Call` button (phone icon) that joins a room for that DM. The other person sees, at the top of that DM's message list, a banner `X is in a call · Join` in the same style as the reply bar, driven by `CallState`. No ringing, no push. Leaving is the same dock.

### Copy
Empty voice room in the sidebar shows nothing extra. Tooltip on a voice room: `Join hangout`. Error when LiveKit is unconfigured: `Voice isn't set up on this server yet.` Error when permission denied: `Den needs your microphone. Allow it in the browser's site settings.`

### Verification
Write `scripts/m3-smoke.mjs` using `playwright-core` with `executablePath: '/usr/bin/chromium'` and Chromium flags `--use-fake-ui-for-media-stream --use-fake-device-for-media-stream`. Two browser contexts log in as `nicholas` and `bob` against the dev server, both join `hangout`, and the script asserts each page shows two tiles, one shows the other's name label, toggling mic on one shows the muted icon on the other within two seconds, and leaving removes the tile. The dev server, seeded users, and password live in the scratchpad README that Fable used: ask Fable if you cannot find it. Take screenshots of the strip, the grid, and the dock to `docs/shots/m3-*.png` at 1440×900 and 390×844 and look at them before declaring done.

Write `docs/M3-NOTES.md` with how to run it and what changed in the contract, then stop.
