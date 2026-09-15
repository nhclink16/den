# Sound-pack verification

Branch: `feat/sounds`. Migration: **0018_sounds.sql**, leaving 0017 to the music
lane. Shared types are in `den-core`; the web schema is generated from OpenAPI.

## Feature checks

- Two server integration tests cover user/server/built-in precedence, per-event
  silence, missing IDs, admin-only server changes, account isolation, CSRF,
  512 KiB and five-second limits, non-audio rejection, all-or-nothing ZIP
  validation, chat installation, partial-pack layering, and offline backup restore.
- A WebSocket compatibility test keeps legacy, explicitly disabled, opted-in,
  and native-ticket sockets open across account and server changes. Later chat
  messages prove legacy streams remain usable; sound updates reach only opted-in
  sockets with the correct account targeting.
- Chromium exercised file replacement, account persistence after reload, named
  packs, admin server defaults, normal channel uploads, preview without install,
  installing a pack and a single sound, focused-room suppression, away-room audio,
  upload/error events and reduced-motion overlap prevention.
- Two real LiveKit clients in an isolated room exercised own join/leave, another
  participant joining/leaving, and a published screen share. Capture media was
  synthetic; the room and signaling were real.
- WAV, MP3 and Ogg Vorbis uploads were accepted by the running server.

Evidence is under `docs/shots/pr/feat-sounds/`. Settings before/after use the same
account, theme, background and 1440×1100 viewport. The before view shows the old
Voice sound switch; the after view shows the new Sounds section. `sharing.png`
shows both attachment cards; `mobile.png` shows Settings at 390×844.

`test-all.mp4` includes the actual browser audio output. The capture taps the
AudioContext that connects to the speakers, records its MediaStream, and muxes
that audio with ffmpeg screen frames. Silent timestamp gaps are filled during
muxing. Its companion `browser-proof.json` records eleven playback starts and
checks that every gap is at least one second after the previous sound ends.

## Build and regression checks

`cargo test --workspace` passed, including 46 API integration tests. Workspace
formatting and Clippy with warnings denied passed. The web build and Svelte
check passed (one existing unused-style warning in Login). The CLI smoke test
passed messaging, reconnect after restart, media upload/range/decode, and agent
token revocation. Both GitHub Actions workflows passed before the final evidence
refresh.

## Reproduce the browser checks

Use a disposable instance at `127.0.0.1:7018`, with
`DEN_ORIGIN=http://localhost:5182`. Run Vite with `vite.sounds.config.ts`.
The scripts expect test accounts `nicholas` and `av_observer`, whose password is
provided through `DEN_PASSWORD`; no credentials are stored in the scripts.
They create disposable rooms and replace the test account’s sound preferences.

```sh
SOUNDS_CAPTURE_BEFORE=1 node scripts/sounds-browser.mjs
# Restart the isolated Vite server to clear its hot-reload module cache.
node scripts/sounds-browser.mjs
node scripts/sounds-sharing-smoke.mjs
node scripts/sounds-call-smoke.mjs
```

With `SOUNDS_CAPTURE_BEFORE=1`, the first script temporarily reads the original
Settings files from commit
`d05e41a` (override with `SOUNDS_BASE`) to take the before image, then restores the
working files in a `finally` block. Restart Vite between captures: swapping
historical components can leave duplicate hot-reload store modules. Run it only
in the isolated worktree.

## Limits of verification

No physical speaker/headphone listening test or Electron/iOS device acceptance
was performed. The terminal bell uses Ghostty’s `onBell` event, but a live host
terminal was not available in this isolated fixture. These checks do not claim
hardware or native-device acceptance. Browser playback and the recorded audio
were verified directly.
