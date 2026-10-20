# Roadmap

Each milestone is shippable on its own. Do them in order.

## M0 — skeleton (done)
Workspace builds, `den-server` serves `/health`, `den health` talks to it, Svelte app scaffolded.

## M1 — text chat, CLI first
- SQLite + migrations: users, sessions, tokens, invites, categories, channels, channel members (for DMs), messages.
- Auth: register with invite code, login, session cookie, bearer tokens for agents.
- REST: channels CRUD (admin), DMs, messages list/create/edit/delete, users list, uploads (chunked, range-served).
- WebSocket `/ws` streaming `Event`.
- OpenAPI via utoipa at `/openapi.json`.
- CLI: `den init` (bootstrap first admin), `den login`, `den channels`, `den send <channel> <text>`, `den read`, `den tail [channel]`, `den upload`, `den token create`, `den bot create`.
- `skills/den/SKILL.md` written against the real CLI.
Done when: two terminals can chat through the CLI, a 41-second clip uploads and streams back with range requests, and an agent can post with a bot token.

## M2 — web client (client done 2026-09-09, deploy pending)
- Login, channel list with categories, message view, composer with markdown, replies, reactions, mentions, typing, presence.
- Two collapsible columns, member drawer, Ctrl+K palette, settings page.
- Unread inbox and quiet-by-default notification prefs.
- Server APIs for replies, reactions, read state, notification prefs, search (FTS5). (Astra)
- Inline video/audio players, image thumbnails, upload progress. (Fable)
- First real deployment: Hetzner box, domain, Caddy TLS, backups. Friends need a URL, not localhost. (Astra)
- Search (SQLite FTS5).
Done when: the group can use it in a browser at a real URL instead of Discord for text.

## M2.5 — iOS feasibility spike
Retired 2026-09-14 without running the call test, and the iPhone app went native. Reopened 2026-10-08: see M5.

## M3 — voice and video
- LiveKit in compose, server mints tokens, hangout room and DM calls.
- Docked cam strip, screen share, device picker, voice activity and PTT.
Done when: a gaming session runs on it with cams.

## M4 — desktop (started 2026-09-14, brief in `docs/M4-DESKTOP-BRIEF.md`)
- One Tauri 2 project for macOS, Windows, Linux: native sessions in the keychain, multi-server switcher, global PTT, native notifications, tray, updater, signed releases.

## M5 — iOS, thin Swift shell (decided 2026-10-08)
The native SwiftUI app (about 7,800 lines, decided 2026-09-14) is retired. It rebuilt every screen, so each feature had to be built twice and the phone fell behind: no Jam, Servers, wallpapers, customizer, music or GIFs. The new app shows the same Svelte SPA the desktop shells use, so a feature built once reaches the phone. Swift does only what a web page cannot on an iPhone:
- APNs push (the server already sends it) and the login token in the keychain.
- Calls, decided by a one-hour test on the iMac with a real iPhone: load the SPA in a bare WKWebView shell, join the hangout, then lock the screen, switch apps, use AirPods and take a phone call mid-call.
  - If the call survives, calls stay in the web page and the shell is about 500 lines.
  - If not (expected), a native call engine taken from the old app (LiveKit Swift, CallKit, background audio, about 1,500 lines) runs the call. The web Join button tells it to join or leave, and the call tiles are the one native screen.
- Step one done 2026-10-11 (#72): a bare shell in `apps/ios-shell` ("Den Shell", `app.denchat.shell`) is built and signed, waiting for the call test. On the simulator WebKit ends the page's mic when the app goes to the background, so the native call engine is the likely outcome.
- Call test run on the simulator instead of a real phone (2026-10-11, Nicholas's call): web calls fail. Locking or switching apps, even for 10 seconds, cuts your mic for everyone, in Den Shell and as a home screen web app alike. Den Shell also stops playing the others after about 27 seconds. So the native call engine is being built, voice first; AirPods, a real phone call and the camera get a short real-phone check at the end.
- Native call engine phase 1 (voice) merged 2026-10-12 (#84): inside Den Shell the call runs in Swift (LiveKit Swift, CallKit, background audio) and the web page drives it through `window.denShell`. On the simulator, with CallKit off (the simulator can't host CallKit calls), both directions keep flowing for 2 minutes locked and in another app. Released in 0.3.9; waiting on a 20-minute real-phone check of CallKit, AirPods and a phone call. Phase 2: native video tiles; then push.
- Den Shell builds in Xcode Cloud and ships to TestFlight as "Den" (`app.denchat.ios`, replacing the old native app, D18, #87). The first build, 0.4.0 (5), went to the internal group on 2026-10-13. Pull requests that touch `apps/ios-shell` get a cloud build-and-test check.
- The old app stays in `apps/ios` until the shell replaces it, then is deleted. PR #55 (Take photo) was closed with it.

## M6 — deploy and integrations
- Link previews with an SSRF-safe fetcher.
- Hermes platform plugin under `integrations/hermes`: a `plugin.yaml` plus `adapter.py` implementing `BasePlatformAdapter`, installed by symlink into `~/.hermes/plugins`. Hermes is the primary agent host.
- OpenClaw channel plugin under `integrations/openclaw`, optional, only if Clanker stays on OpenClaw.

## M7 — layouts and plugins

### M7a — live objects + canvas plugin (started 2026-09-13, brief in `docs/M7A-CANVAS-BRIEF.md`)
One core primitive (a live JSON object attached to a message, synced over the existing WebSocket) and a minimal plugin surface, proven by a tldraw canvas plugin shipped in the default build. Done 2026-09-13.

### M7b — den-host, access grants, terminal plugin (done 2026-09-13, brief in `docs/M7B-HOST-BRIEF.md`)
A dial-out host binary, a grant model where requests and approvals are cards in chat, and a Ghostty-rendered terminal as the second plugin. Seams left for a direct WebRTC path and libghostty-vt screen-state sync.

### M7c — call layouts (done 2026-09-14, brief in `docs/M7C-LAYOUT-BRIEF.md`)
Every stream visible at once, drag, resize, pin, pop-out, presets, per-room saved layouts.

- Multiple simultaneous screen shares visible at once, always. Drag, resize, pin, pop-out, per-room saved layouts. See `docs/IDEAS.md`.
- Plugin system in the spirit of Herdr's: small documented surface, vibecodeable in one sitting. Client panels, tile types, slash commands, message renderers; server hooks over the existing API.
- Open-source release: license headers, contributor notes, public repo.

## M8 — portable (agreed 2026-09-14, see `docs/HOSTING-PLAN.md`)
- Prebuilt release binaries and a host `install.sh`; friends never need Rust.
- Offline `den-server export` / `import` with a manifest, run with the server stopped; restore drills against the real server.
- Bounded instance name, shown where the client says "Den".

These three items are complete; acceptance is recorded in [M8 notes](M8-NOTES.md).

- Multi-instance client design settled; ships in the Tauri shells (M4), not the browser.
- Later: admin "Export server backup" download after a privacy review; hosted multi-tenant with one LiveKit per tenant; game-server plugin.

## M9 — appearance and themes (done 2026-09-14, brief in `docs/M9-THEMES-BRIEF.md`)
Shared theme schema in den-core, nine built-in themes, fonts, radius, density, a live editor with export and import, synced per user. Done before the desktop and iOS apps so they consume the same JSON.

## Dictation (started 2026-09-14, brief in `docs/DICTATION-BRIEF.md`)
On-device speech to text in the composer: iOS 26 SpeechAnalyzer on the phone, Whisper in WebAssembly on web and desktop. Audio never leaves the device.

## Shipped since 2026-09-24 (desktop 0.3.2)
Electron replaced Tauri for the desktop app (0.3.0 was the first Electron release; 0.3.2 is the first one whose updates the app can actually verify). Also shipped:
- Hidden title bar on Windows and Linux, and an in-app screen-share picker.
- 720p screen-share layers for phone viewers.
- Spotify Jam moved into the hangout.
- The Servers page with a live Minecraft relay.
- A Desktop app download section in web Settings.
- Activity status, wallpapers, admin member tools and the live Appearance panel.
- 0.3.3: GIF profile pictures, banners and wallpapers stay still until hovered, and GIF avatars can be any shape.
- 0.3.4: GIF search from KLIPY in the composer, and GIF pictures, banners and wallpapers picked from it. Everything loads straight from KLIPY, and Den stores only KLIPY's links, per KLIPY's rules. The server reads the key from `DEN_KLIPY_KEY`. Production key requested 2026-10-09; the test key allows 100 searches an hour until KLIPY approves it.
- 0.3.5: fixes from the 2026-10-11 usability pass. On the iPhone, no stuck zoom after typing, a picture viewer you can close, clip first frames, and long-press for react, reply and copy. Search looks everywhere, the thread strip hides when empty, and a thread's newest reply shows under its message in the room.
- 0.3.6: opening a room with unread messages starts at the first unread, and the room counts as read only once you reach the newest message.
- 0.3.7: the small usability fixes. On phones, long messages get the box's full width, the expanded call shows one row of buttons, and Appearance opens full screen. Settings groups Machines, Access, API keys and Agents under Advanced. Thread titles show names, the Resolved list keeps up, the sidebar call bar fits on one row while sharing, and Servers Playtime shows Den names for players online.
- 0.3.8: a searchable emoji picker for reactions, the message box and the status emoji, with recent emoji first and a bottom sheet on phones. The emoji list loads only when a picker first opens.
- 0.3.9: the web half of native calls in Den Shell (#84). No change outside Den Shell.
- 0.4.0: revamp slice 1 (Quiet direction, D16/D17, #86). A plain grey "Neutral" theme with a blue accent is the default for anyone who never changed Appearance; amber Den stays as a theme. The accent is its own setting, normal text drops the code font and the slashed zero, buttons, pills and switches share one style, and the phone message box gets a round + menu.
- 0.4.1: revamp slices 2 and 3 (#89, #90, #91, D20/D21). Phones open on a room list with each room's newest message; rooms push in with a back chevron. Rooms get Lucide icons an admin picks (migration 0026, a nullable `channels.icon`), and each person chooses icons, # or nothing. Messages are bubbles with a bubble-shape setting and per-person colours (everyone's, only mine, or plain); open threads sit behind one pill; code blocks use the full width.
- 0.4.2: revamp slice 4, the call (#93, D22). FaceTime-style tiles and round buttons with a red Leave; while you chat the call shrinks to a pill (faces, who's talking, mute, leave) instead of the phone video strip; the call now shows "Reconnecting…" when LiveKit resumes a dropped connection.
- 0.4.3: revamp slice 5, settings (#95, D23), the last slice of the redesign. Grouped lists like iPhone Settings, drill-in on phones, Appearance as a page with theme cards drawing today's app; no setting removed or changed. Also Den Shell UI checks in Xcode Cloud against a made-up Den from the branch (#96), and the live server now has Den's APNs key (D24) for push once Den Shell registers devices.
- 0.4.4: push notifications in Den Shell (#98, D25): permission after sign-in, mentions/DMs/followed rooms as alerts, taps open the room or thread, badge = unread count. Den Shell build 25 in TestFlight. Call ringing waits for the real-phone call test (D4).
- 0.4.5: redesign clean-up (#101, D29): on phones the open call and threads fill the screen and Search is a button; the phone home shows only the call pill during a call; call buttons stay on one row and the sidebar call bar is a tidy two-row panel; room icon and name changes show live for everyone (`channel_updated`). Also Den Shell on iOS 17 (#100): its web fix ships here, TestFlight build 29+ installs on iOS 17.
- 0.4.6: phone alignment (#103, D30): the composer's + sits level with the message box (it was 4px low since 0.4.0), a photo waiting to send sits on the box instead of over the +, and a thread's back arrow is where a room's is. Measured with a new alignment script, now part of phone QA.
- 0.4.7: Den on iPhone feels less like a website (#105, #106, D31, D32). Haptics when you send, react, long-press, join or leave a call, mute, hit an error or flip a setting. No grey tap flash, no text selection on controls, no whole-page bounce, pressed states everywhere, no sticky hover, and bottom sheets pull down to close. Reply to a Den notification from the lock screen (Face ID first); the shell signs with the page's own session, checks it is the person the alert was for, and shows "Reply not sent" with the text if it fails.
- 0.4.8: iPhone polish, Den's own look built the iPhone way (#108–#114, D33, D34). Den Shell draws edge to edge, the keyboard keeps the top bar, Safari's form bar is gone, and a dead server shows "Can't reach Den". One iPhone top bar everywhere; home with a compose button, grouped Inbox, rebuilt Search. Own messages in the accent, a long-press menu with blur and quick reactions. Profiles as bottom sheets, your profile saves by itself, a FaceTime-style call screen and a slim call bar. Settings reads like iOS Settings. Sign-in starts from the launch mark, and Den asks before iOS's notification prompt. Not yet: translucent bars, the native photo picker, Inbox swipe, a native share for "Den on your computer".
- 0.4.9: fixes after 0.4.8 (#116–#118, D35, D36). The Den mark lost its hairline (one outline, no even-odd) on web, boot, favicon, Den Shell and the launch screen; sign-in has no glow; the desktop sidebar drops the connection dot, its server switcher opens in the top layer, and the hangout card lines up with the room rows. On phones, Appearance keeps a live preview pinned under the bar. A new theme fills both light and dark when one theme was used for both, and your own theme's corners, density and fonts save as you go. Avatar shape and the online indicator now sync with the rest of Appearance; older clients that omit them keep the stored values.

## Backlog (decided or asked for, not built)
- **Servers follow-ups:**
  - modpack download: the relay rebuilds a Modrinth `.mrpack` when server mods change
  - sleep-when-empty via lazymc
  - a 7 Days to Die server, runnable only while Minecraft is down
  - Spark mod for TPS and CPU (needs a Minecraft restart)
  - the playit join address on the page
- **MW2 1v1s:** a private IW4x server (`sv_lanonly 1` plus a password), a Start 1v1 button on the Servers page, and a hangout card with an `iw4x://` Join link. Waiting on Nicholas owning MW2.
- **Landing page at `/`, app at `/app`** (deferred 2026-09-16).
- **Mac code signing:** needs an Apple Developer ID. Until then Mac builds are unsigned and Mac auto-update does not work.
- **RCON hardening:** Minecraft RCON listens on all interfaces with its password in the system unit. Bind it to localhost (needs approval: system unit change).
