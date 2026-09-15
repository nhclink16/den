# Terminal and uploads lane

Branch `fix/terminal-uploads`. No deployment or release.

## Behavior

- New terminals record only when the machine owner's account preference is enabled. The header's **Record session** checkbox changes the current session and remembers the choice for that machine. Settings, Machines can change the default for future sessions. Only the machine owner can change either value.
- Turning recording off removes the current session's partial recording while holding the same write lock used for output and session end. Output continues to viewers. Off sessions produce no recording upload or play button. Existing completed recordings retain their access checks and replay format.
- Recording files live in the server's configured `DEN_UPLOADS` directory, `data/uploads` by default. They contain timestamped terminal output and dimensions, capped at 64 MiB. Live recording uses `<session-id>.recording`; completed recordings use their upload ID.
- Ghostty remains pinned to 0.4.0. Its `hasMouseTracking()` and `getMode()` queries determine whether to send mouse reports. The renderer handles presses, releases, drags, and wheel input. Shift allows text selection. Reports follow [XTerm's mouse protocol](https://invisible-island.net/xterm/ctlseqs/ctlseqs.html#h2-Mouse-Tracking), including SGR and byte-encoded legacy coordinates. Viewers cannot send input, and double Escape still releases terminal focus.
- Uploads live in the account/instance store, keyed by channel. Leaving the composer keeps the transfer, progress, completed attachment, and errors. An arrow marks a room while files are transferring. Removing an attachment cancels further chunks and completion. Logging out clears the account's queue.
- A reload ends these browser uploads. File handles and pending attachments are not restored after reload. This change does not claim resumable browser uploads across reloads. The CLI's existing resume support is unchanged.

## Review scope

Shared types are in `den-core`, the web schema is generated from OpenAPI, and migration `0016_terminal_recording.sql` stores per-user/per-host preferences. Migration 0015 is reserved by tonight's AV lane; 0016 avoids a numbering collision. No applied migration was edited.

## Verification

Verified on the isolated API at `127.0.0.1:7013` and Vite at `localhost:5183`, with a temporary enrolled host. The shared development server was not restarted or reconfigured.

- All 41 server API integration tests passed, including recording opt-in, discard, ownership, persisted defaults, old recordings without the new state field, portable backup/restore, and terminal grant revocation.
- Workspace Clippy passed with warnings denied; Rust formatting passed.
- Web type check passed with zero errors and warnings. Production web build passed with the existing large-chunk warning.
- The browser smoke ran Chromium at 1440×900 with the same account, light theme, viewport, room names and scenario before and after. It checks live Herdr pane and tab focus through Herdr's CLI, then checks mouse bytes arriving at a real PTY. SGR press, drag, release, and wheel reports passed. A legacy coordinate above ASCII 127 arrived as the original bytes. Tracking-off scrollback passed.
- The upload check held chunk requests at controlled checkpoints. After leaving the room, 8 MiB committed in the background. Returning showed the same attachment at 33%; it completed and was sent to the original room. Before the change the pending attachment count was zero; afterward it was one. Removing an attachment during a chunk request prevented completion.
- Default-off session end had no recording upload and no play control. Mouse and keyboard toggling both passed. Enabling recording from the header persisted the account preference, produced a replay, and enabled recording for the next session. Playback starts at the first recorded output, skipping time before opt-in.

Repeat on disposable data, using the matching host binary and a server whose `DEN_ORIGIN` matches the client:

```sh
export CARGO_TARGET_DIR=/mnt/storage/den-terminal-target
cargo test --locked -j 1 -p den-server --test api
npm --prefix apps/web run check
npm --prefix apps/web run build
DEN_SMOKE_URL=http://localhost:5183 \
DEN_SMOKE_HOST_BIN=/mnt/storage/den-terminal-target/debug/den-host \
DEN_SMOKE_PASSWORD="$DEN_SMOKE_PASSWORD" node scripts/terminal-uploads-smoke.mjs
```

This machine's Rust checks used `--config 'profile.dev.package.den-server.debug=0' --config 'profile.test.package.den-server.debug=0'` to reduce memory use alongside the other lanes. `BEFORE=1` captures the known failures against the original frontend and server instead of asserting the fixed behavior.

## Visual evidence

Images and silent H.264 MP4s are in `docs/shots/pr/fix/terminal-uploads/`.

| Scenario | Before | After |
| --- | --- | --- |
| Recording header | [Before](../shots/pr/fix/terminal-uploads/recording-before.png) | [After](../shots/pr/fix/terminal-uploads/recording-after.png) |
| Ended session | [Before](../shots/pr/fix/terminal-uploads/ended-before.png) | [After](../shots/pr/fix/terminal-uploads/ended-after.png) |
| Machines settings | [Before](../shots/pr/fix/terminal-uploads/machines-before.png) | [After](../shots/pr/fix/terminal-uploads/machines-after.png) |
| Click in Herdr | [Before](../shots/pr/fix/terminal-uploads/mouse-before.png) | [After](../shots/pr/fix/terminal-uploads/mouse-after.png) |
| Upload while away | [Before](../shots/pr/fix/terminal-uploads/upload-away-before.png) | [After](../shots/pr/fix/terminal-uploads/upload-away-after.png) |
| Upload after return | [Before](../shots/pr/fix/terminal-uploads/upload-return-before.png) | [After](../shots/pr/fix/terminal-uploads/upload-return-after.png) |
| Herdr interaction | [Before video](../shots/pr/fix/terminal-uploads/herdr-before.mp4) | [After video](../shots/pr/fix/terminal-uploads/herdr-after.mp4) |
| Upload navigation | [Before video](../shots/pr/fix/terminal-uploads/uploads-before.mp4) | [After video](../shots/pr/fix/terminal-uploads/uploads-after.mp4) |

The additional `recording-on-after.png` and `replay-after.png` show opt-in and playback.

## Not verified

Physical mobile devices, screen readers, and native clients were not exercised. The browser smoke verified SGR and legacy mouse reports; X10, UTF-8, urxvt and pixel-coordinate modes were implemented but not exercised against a real application. LiveKit was not configured in the isolated instance, so the upload check switched text rooms; call expansion uses the same store but was not exercised in a live call. Reload recovery is deliberately outside this change.

