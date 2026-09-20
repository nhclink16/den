# Voice controls

The account owns a map of microphone and camera adjustments keyed by browser
media device ID. `GET /users/me/voice` loads it; `PUT` merges device entries under
the existing server write lock. `voice_preferences_updated` uses the same private
WebSocket filtering as appearance updates. Login and reconnect reload the account
value. Different accounts and native server stores keep separate maps. Camera
background is one of those account-backed, per-device adjustments: `none`, `blur`,
or `light_blur`.

Browser device IDs are scoped to a site and browser profile. The account carries
the map to other sessions, but a browser that reports a different device ID starts
with defaults. The browser's `default` microphone ID stores adjustments for the
system default input. Existing device selection, PTT, join-camera and sound
preferences retain their existing local storage behavior.

## Media path

`av.ts` owns the microphone processor and capability-to-constraint conversion.
The outgoing path is microphone → GainNode → MediaStreamAudioDestinationNode →
LiveKit. The SDK gets the processor before publishing, after assigning its audio
context. Mute and PTT still gate the captured track. Slider input changes gain
immediately; releasing it saves the setting. Settings uses the same processor for
its private microphone test, with its meter connected after gain.

The three processing switches use exact constraints on the captured source track.
Some browsers advertise switchable processing but reject changes while capturing.
Den reports that rejection and restores the control without saving it or restarting
the call. A settings request must succeed before its new value is stored.

Camera choices come from `getCapabilities()`. Exact resolution and frame-rate
constraints expose rejected combinations instead of silently selecting another
mode. The preview reports actual `getSettings()` dimensions and frame rate.
Brightness, contrast and saturation only appear when the camera advertises a
non-empty numeric range. Mirror changes CSS on local camera previews only.

Settings borrows a running camera track. When there is no published camera it
owns a private capture. Closing Settings or pausing the preview releases only
owned tracks. Leaving a call still releases the published tracks.

Background blur is a LiveKit video processor on the camera track. The room's
video capture defaults carry it, which is the one hook that also covers the
tracks the SDK recreates by itself; `setProcessor` and `stopProcessor` handle a
mid-call toggle. `stopProcessor` re-applies the constraints the SDK captured with,
which are only the device ID, so turning blur off has to put Den's own resolution,
frame rate and picture settings back afterwards. That happens after the reactive
blur flag is refreshed, because the reader that resolves the camera behind the
processor is keyed on it and a stale flag skips the restore. Both the published
track and the Settings preview show the processed output. The
published camera gets it from the SDK, which replaces the sender track and the
attached elements. Settings' borrowed preview gets it from the same publication.
Settings' owned capture has no `LocalVideoTrack`, so it drives a processor of its
own against a hidden video element, and destroys it under the same ownership rule
that governs the track.

Blur hides the real camera. Once a processor runs, the published track is a
generator or canvas track with no device, no capabilities and no useful settings,
so capabilities, `applyConstraints`, `getSettings` and the device ID that keys
saved camera preferences all read the processor's `source` instead. Without that
split the resolution, frame-rate and picture controls empty out and every camera
collapses onto the `default` preferences key.

The frame-rate fallback is Den's. `@livekit/track-processors` 0.8.0 ignores
`maxFps` on the modern transform path, so Den caps camera capture to 30 fps before
starting the processor. Over a window of processed frames, mean total work above
the frame budget steps that cap down through 24 and 15 fps. Version 0.8.0's
`processingTimeMs` double-counts synchronous segmentation, so Den uses its
`filterTimeMs`, which spans segmentation and rendering in that exact version. The
cap is a maximum, not an exact rate, so the saved preference is untouched and
returns when blur is turned off. Below 15 fps blur turns itself off and says so.
It never steps back up on its own. A camera change restarts the processor and the
ladder together.

Blur uses the modern insertable-track path only. The package's canvas fallback can
block the renderer during segmentation and cannot clean up an initialization that
is cancelled in version 0.8.0. Den keeps the pinned package's synchronous support
probe locally, without the fallback clause, so the processor code stays out of the
initial bundle. Where it is unsupported, Settings shows disabled choices and a
reason and the call control is hidden. A segmenter that fails to start after the
probe passed retries the same camera without the effect, reports the fallback, and
clears the effect only after the plain camera succeeds. A permission or capture
failure on both attempts preserves the saved preference.

MediaPipe's WASM and model are served by Den. The package fetches them from
jsdelivr and storage.googleapis.com by default, which a self-hosted app has no
business doing. `apps/web/public/blur/selfie_segmenter.tflite` is 244 KB and lives
in git next to the vendored font; the WASM is 19 MB across four files and is staged
from the exact direct `@mediapipe/tasks-vision` dependency into
`public/blur/wasm-0.10.14` by `scripts/blur-assets.mjs`, which both `npm run dev`
and `npm run build` call. That directory is git-ignored. The WASM directory and
hash-versioned model URL receive a one-year immutable cache header; Caddy uses
Zstandard or gzip for the roughly 9.9 MB first activation. Complete upstream
licences, LiveKit's NOTICE and model provenance ship under `public/licenses`.

## Extension points

Two seams are marked by name in `MicrophoneGain.init`. SOUNDBOARDS is the mixing
point: an extra source connects into the gain node and rides out with the voice,
with its own gain node for its own level, and never also into `context.destination`
or the clip is heard twice and recaptured by an open microphone. VOICE MODS is the
insertion point: effect nodes go between `input` and `gain`, connected as
input to effect to gain, held as fields so `destroy` can disconnect them, with
`gain` left last so the input-gain slider keeps the final word. Neither is
implemented. Both are per-user client-side mixing, so neither needs a server
change; a shared soundboard that others hear on their own timing would.

Background blur is implemented. `@livekit/track-processors` is pinned exact at
0.8.0 alongside `livekit-client` 2.15.6, and `@types/dom-mediacapture-transform`
is its type peer. Use `BackgroundProcessor({ mode: 'background-blur' })`; the
`BackgroundBlur()` form the older LiveKit examples show is deprecated in 0.8.0.
A virtual background is the same processor with a different mode and would need an
image to composite, a place to store it and `switchTo` for artifact-free changes.

## Verification

The API integration test covers missing authentication, cookie CSRF, persistence,
merging devices, invalid values, owner-only WebSocket delivery and account privacy.

Run the browser smoke against an isolated dev database with `nicholas` and
`av_observer`, a unique voice channel named `av-verification`, and working LiveKit:

```bash
ffmpeg -f lavfi -i 'sine=frequency=440:sample_rate=48000:duration=90' \
  -filter:a 'volume=0.1' /tmp/av-microphone.wav
DEN_SMOKE_URL=http://localhost:5178 \
DEN_SMOKE_PASSWORD='<dev password>' \
DEN_SMOKE_AUDIO=/tmp/av-microphone.wav node scripts/av-smoke.mjs
```

The test uses Chromium synthetic media. Physical camera image quality, real
microphone processing quality, Bluetooth devices, Safari, native apps and
screen-reader output are not verified here. Chromium's synthetic camera does not
advertise brightness, contrast or saturation; the hidden-control path is tested.

Background-blur acceptance evidence, red/green proof, review findings, exact-head
CI links and remaining platform risk live in `docs/AV-EXTENSIONS-QA.md`.
`scripts/av-blur-smoke.mjs` covers the processor primitive, self-hosted assets,
strength switching, the rate ladder and failure cleanup. `scripts/av-smoke.mjs`
covers owned and published tracks with a real LiveKit room and observer.
`scripts/av-background-edge.mjs` covers model failure, denied capture and the
unsupported-browser UI. The processor package is a lazy chunk rather than part of
every app load.

Verified on 2026-09-15:

- All 41 server API integration tests passed. Clippy passed with warnings denied.
- Svelte/TypeScript checks passed without warnings; production build passed with
  the existing bundle-size advisory.
- The browser smoke passed with two accounts and a second same-account session.
  Processed outgoing RMS was 0 at zero gain and approximately 0.0177 at 200%.
  The observer received audio packets and decoded camera frames.
- 720p → 1080p and 24 → 60 → 30 fps retained the participant, track and publication
  IDs. This checks capture settings; SFU subscriptions and encoding can deliver
  a lower resolution or frame rate for bandwidth and tile size.
- Local-only mirror, remote mute/unmute, microphone switching, pause/resume,
  Settings navigation, owned-capture cleanup, account events and reload passed.
- `scripts/av-permission.mjs` passed a denied-camera retry while the mic stayed
  available, cleanup after leaving Settings, and a 20 fps camera with no supported
  24/30/60 choice. Run it with `DEN_SMOKE_FPS=20` to exercise that capability limit.
- Chromium's synthetic microphone rejected all three exact live processing
  changes. The smoke verified error reporting, control rollback and call continuity.
  Successful live DSP switching on physical hardware remains unverified.
- Matched 1440×1000 screenshots, a 390×844 view and a seven-second silent recording
  are under `docs/shots/pr/feat/av-settings/`. The new device controls fit the narrow
  viewport. The recording was encoded with ffmpeg and its frames inspected.

For an isolated Vite client, run `npm --prefix apps/web run dev -- --config
vite.av.config.ts`; it proxies to port 7014 and serves on 5178. Use a separate
`DEN_DB` and `DEN_UPLOADS` and `DEN_ORIGIN=http://localhost:5178`. The smoke never
creates or deletes channels or sends chat messages. Its observer and unique voice
room must already exist in the isolated database.

The capability conversion follows the browser's
[constraints model](https://developer.mozilla.org/en-US/docs/Web/API/Media_Capture_and_Streams_API/Constraints).
