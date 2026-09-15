# Call and stream fixes

Branch `fix/call-streams`, based on `477761e`. Worktree `/mnt/storage/wt-streams`.

## Behavior

- Each remote person's tile menu and member row share a volume slider and a
  "Mute for me" toggle. The setting applies to their microphone and shared audio
  on this device. Unmuting restores the previous volume. Zero shows a crossed-out
  speaker. Microphone mute and whole-call sound controls remain separate.
- Volume is saved by server origin and Den account ID in `den.call-volume`.
  Reconnecting creates new LiveKit identities but retains the person's volume.
  Newly subscribed audio, including after turning call sound back on, uses it.
- Each local share has a visible Stop button. The dock's "Your shares" menu lists
  the same individual stops. The main sharing button stops all local shares.
- Capture labels and monitor/window/browser glyphs travel in LiveKit attributes,
  including to late joiners. Raw window handles become "Window"; missing labels
  become "Screen", "Window", or "Browser tab". Useful titles remain readable.
- The call strip's bottom separator resizes from 100px to the column height minus
  160px. Up/Down change it by 20px. Home/End choose the limits. Double-click or
  Enter resets to 160px. The height is saved by server and room.
- Voice rooms appear before text rooms and categories, ordered by position.

The capture API provides a surface kind, not an application's icon. Den uses
matching monitor, window and browser-tab glyphs. It cannot recover a title when
an operating system supplies only a handle. See the browser's
[displaySurface documentation](https://developer.mozilla.org/en-US/docs/Web/API/MediaTrackSettings/displaySurface).

## Verification inventory

| Request | Browser checks | Visual evidence |
| --- | --- | --- |
| Per-person volume | Keyboard slider input changes received microphone and share audio element volumes; local mute keeps publisher microphone enabled; unmute restores 35%; member control shares state; reload and deafen retain volume | Tile menu and member row, before and after |
| Per-share stop | Stop one through its tile; publisher and viewer lose only that share; its video/audio tracks end and the second capture remains live; dock stop and main stop-all also exercised | Share controls before/after; silent stop-one recording |
| Share names | Real getDisplayMedia with fixture labels `window:37438947` and `Helium — docs`; real publication and subscription; labels sanitized locally and for viewers; source parsing unit tests | Same three shares before and after |
| Resizable strip | Pointer drag; arrow keys; limits; double-click and Enter reset; saved height after reload | Same chat before/after; silent drag recording |
| Voice-first sidebar | Hangout precedes general | Same sidebar before and after |

The smoke uses a private SQLite snapshot on port 7012 and this worktree's Vite
client on 5182, with unique voice-room IDs. It does not run against the public
service. Screenshots use 1440x900 and the same saved light appearance; extra mobile
checks use 390x844. Media comes from Chromium fake devices and real WebRTC
connections. A synthetic audio track is added if fake display capture has none.
No microphone or application-window content from a person is recorded.

Reproduce with an isolated seeded server and `nicholas` and `bob` test accounts:

```bash
npm --prefix apps/web run check
npm --prefix apps/web run build
node --experimental-strip-types --test scripts/streams-source.test.ts
DEN_SMOKE_URL=http://localhost:5182 DEN_SMOKE_PASSWORD=<dev-password> \
  DISPLAY=:92 node scripts/streams-smoke.mjs
```

Start Xvfb at 1440x1000 for the headed recording pass. Omit DISPLAY for the
functional headless pass. `DEN_STREAMS_PHASE=before` captures the baseline when
running the client at the base commit. The script keeps credentials out of its
output and expects a throwaway database.

Not verified: physical speakers, Bluetooth, native desktop capture pickers,
Safari/iOS, screen-reader speech output, and a human multi-device call. Browser
source metadata in these captures is controlled test input. No claim is made
that a browser can discover application names or icons that its capture API
withholds.

## Results

- Web build passes. The existing large-bundle advisory remains.
- Svelte/TypeScript check: zero errors and warnings.
- Five source-label and existing call-layout unit tests pass.
- `streams-smoke.mjs` passes all checks in the table plus mobile control bounds,
  camera/share fit at 100px, and a usable volume popover at that height.
- Existing `m7c-smoke.mjs` passes, including native Document PiP, popup fallback,
  share quality in the popped-out tile, drag/resize, pinning, three-share limit,
  sender stats, and mobile Focus. Its stop locator now uses the readable label.
- The final screenshots were opened and reviewed. ffprobe confirms silent H.264
  recordings with real video frames. Start/end frames show the share disappearing
  while the remaining video continues.

The inherited wallpaper scales its background layer to 106%, which contributes
12 hidden pixels to the mobile root scroll width. The actual shell, call strip,
resizer, menus and dock fit the viewport; those bounds are checked separately.

## Merge integration

The UI review lane increases call-button spacing. Combined with the new shares
menu, that clipped Leave and squeezed out the room name in the sidebar. The dock
now wraps when needed, retaining the room label and every button. The streams
smoke checks all dock control bounds with multiple shares active. The combined
UI preview passes the full streams smoke, including mobile.
