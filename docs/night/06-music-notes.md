# Den DJ, YouTube queue

Part A only. Spotify Jam is not implemented.

## Playback

`den-server` owns one persisted queue per voice room. A bounded `yt-dlp`
subprocess resolves metadata and a fresh audio URL. ffmpeg decodes it and encodes
48 kHz stereo Opus at 128 kbit/s. The small `den-dj` helper publishes that Opus
stream through the official LiveKit Go SDK, with no second encode. The helper
authenticates with a room-scoped token. Its stable identity is
`den-dj-ROOM_ID` and its name is the instance name followed by DJ.

The Go helper is needed because LiveKit's Rust audio source accepts raw PCM;
the Go SDK supports already encoded Opus. The implementation follows the
[official reader-track publishing API](https://github.com/livekit/server-sdk-go#publishing-tracks-to-room).
The Go SDK is pinned to v2.18.1. yt-dlp is a separate executable pinned by the
installer to 2026.08.19, so it can be replaced independently when YouTube changes.

Only HTTPS YouTube video URLs are accepted. Playlists and live or over-six-hour
videos are rejected. Metadata cannot turn the resolver into a generic URL
fetcher: the decoded stream must use YouTube's googlevideo CDN, and artwork must
use YouTube's image CDN. Resolver output, runtime, retries, and queue length are
bounded. Raw resolver errors and signed media URLs are not sent to users or logged.

Queue mutations serialize with other server writes and emit a complete
`music_queue_updated` WebSocket snapshot. Revision and sample timestamps prevent
an older response from replacing newer state. Pause and seek cancel the existing
pipeline and resume the same queue row at the requested position. Skip and
removal cancel only the current pipeline. Failed resolution or playback leaves
a visible failed row and advances to the next usable track. A server restart
preserves the queue and leaves an interrupted track paused at its last saved
position. Tracks interrupted during metadata lookup resolve when playback resumes.

## Controls

The call dock's music-note button opens the queue panel. It includes artwork,
progress and seek, pause/play, skip, remove, and both drag and keyboard reorder.
Anyone with a Den account can manage a voice room's queue. The voice-room
composer offers to queue a pasted YouTube video. Messages remain unavailable in
voice rooms.

The DJ uses the participant volume component from PR #2 in its tile, member row,
and queue panel. All three change the same saved value. There is no separate
music-volume store or playback path outside the call. Ducking applies a 12 dB
reduction to that saved value while a human participant speaks and restores it
700 ms after speech stops. The checkbox controls ducking on this device.

## Install and run

No deployment or release was performed. On the server, install ffmpeg with
libopus, Python 3, and Node.js 22 or later. Build tooling requires Go 1.27.1.
From the checkout:

```sh
scripts/install-music.sh /path/to/bin
```

The script verifies the pinned yt-dlp download checksum and builds den-dj from
its locked Go module. Put that directory on den-server's PATH, or set
`DEN_YTDLP` and `DEN_DJ_BIN` to the absolute executable paths. `DEN_FFMPEG` can
select an ffmpeg executable when it is not on PATH. Existing LiveKit URL/key/
secret configuration is used. No extra LiveKit ingress service is needed.

The new migration adds `music_rooms` and `music_queue`; existing migrations are
unchanged. The queue is included in normal SQLite exports.

## API and agents

The shared types and generated OpenAPI/TypeScript contract define:

- `GET /rooms/{id}/music`
- `POST /rooms/{id}/music/queue` with `{ "url": "https://youtu.be/VIDEO_ID" }`
- `DELETE /rooms/{id}/music/queue/{track_id}`
- `POST /rooms/{id}/music/skip`
- `POST /rooms/{id}/music/pause` with `{ "paused": true }`
- `POST /rooms/{id}/music/seek` with `{ "position_seconds": 45 }`
- `PUT /rooms/{id}/music/queue/order` with `{ "ids": ["..."] }`. Include every
  non-current row exactly once. A stale list is rejected instead of losing rows.

```sh
den music add https://youtu.be/jNQXAC9IVRw
den music queue
den music skip
den music --room hangout queue
```

Omit `--room` when the instance has one voice room. `den tail` includes queue
events and filters them by room when a room ID is supplied.

### Older clients

Music events require `GET /ws?music=true`. Updated web and CLI clients opt in;
older web, desktop, iOS, CLI, and agent clients keep the existing stream without
the new enum variant. This also protects older Rust clients whose `Event`
deserializer rejects unknown variants. The queue integration test keeps a legacy
socket open during mutations and verifies it receives a subsequent typing event
without receiving any music events. Existing web and iOS handlers also ignore
unknown event tags, but compatibility does not depend on that behavior.

## Verification

The server integration tests cover queue ordering, skip events, member access,
cookie CSRF, URL restrictions, stale reorder rejection, and failed metadata
resolution leaving the remaining queue intact. They replace only the resolver
executable, not the REST or WebSocket implementation, and do not test yt-dlp.

The browser smoke uses an isolated server and two Chromium listeners. Its input
URLs resolve through the pinned real yt-dlp binary, then pass through the real
ffmpeg/Go/LiveKit path. Chromium's fake microphone input supplies a tone for
repeatable active-speaker ducking checks. It verifies received Opus audio energy,
queue synchronization, existing volume/mute, ducking, reorder, pause and seek,
voice-room paste, mobile bounds, volume retention on rejoin, skip, and natural
end-of-track advance. It checks that transport changes leave one DJ audio element.
This caught a notification/playback lock deadlock and stale detached audio elements;
both were fixed before the final run.

The branch integrates the names/mentions, portrait, desktop QA, orientation layout,
and navigation changes through PR #10. No conflicts required manual resolution.

Local checks passed:

- `cargo test --workspace`: 46 API tests plus the workspace unit tests. The two
  music tests were rerun after the playback-lock fix and passed.
- `cargo clippy --workspace --all-targets -- -D warnings`, formatting, and SQLx
  prepare/check against all migrations.
- Web type check and production build. One pre-existing unused `.small` selector
  warning remains in `Login.svelte` from the names/mentions work.
- Go vet/build and a real install into a private bin directory, including the
  pinned yt-dlp checksum check.
- M1 CLI smoke, with its startup wait extended from five to sixty seconds in a
  temporary copy for this busy host. All chat, reconnect, upload/decode, and bot
  revocation checks passed; the repository smoke script is unchanged.
- Music CLI add/queue/skip against the isolated server, and the real browser music
  smoke described above. `verification.json` records received Opus audio energy
  and the measured ducked volume (0.087916 from a saved level of 0.35).
- Restarting the isolated server preserved queue rows, paused state, and saved
  playback position.
- The existing streams smoke on current main, using the private fixture names:
  participant volume/mute/rejoin/deafen, single/all share cleanup, labels, dock
  resizing, and mobile menus all passed.

Screenshots use 1440×900 with the same default light appearance. The baseline is
`d05e41a`. The extra mobile image uses 390×844. The queue recording is silent.

To repeat the browser smoke, use an isolated Vite/server pair with working LiveKit
and the installed media tools. Create `nicholas` and `bob` with the same fixture
password. Set `DEN_SMOKE_URL` and `DEN_SMOKE_CREDENTIALS` (a private JSON file with
`password` and voice-channel `room`). Pause and empty that test room's queue through
the API, then add these URLs in order:

1. `https://youtu.be/dQw4w9WgXcQ`
2. `https://youtu.be/jNQXAC9IVRw`
3. `https://youtu.be/M7lc1UVf-VE`

Run `node scripts/music-smoke.mjs`. Set `DISPLAY` to a private Xvfb display for the
silent recording, and `DEN_SMOKE_AUDIO` to a non-silent WAV file for the real
microphone ducking check. The script consumes and changes this fixture queue.
The recorded final run used both settings.

Physical speakers, Bluetooth, Safari, native iOS background playback, and screen
reader speech are not verified. iOS uses the same LiveKit audio participant path;
this PR adds no iOS queue controls. YouTube availability depends on the server's
network and yt-dlp; videos requiring account access are not supported.
