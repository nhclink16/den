# Voice controls

The account owns a map of microphone and camera adjustments keyed by browser
media device ID. `GET /users/me/voice` loads it; `PUT` merges device entries under
the existing server write lock. `voice_preferences_updated` uses the same private
WebSocket filtering as appearance updates. Login and reconnect reload the account
value. Different accounts and native server stores keep separate maps.

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

## Extension points

Soundboards can mix a per-user source into the gain node. Voice mods can insert
a processing node between the microphone source and gain. Neither is implemented.

Background blur is deferred so the stretch goal does not delay the required PR.
A future `videoProcessors` hook belongs at camera track creation and processor
replacement, using LiveKit's `setProcessor`. Both published video and the settings
preview must consume its processed track. It would need a pinned processor package,
capability checks, a measured frame-rate fallback, and cleanup on device changes.

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
