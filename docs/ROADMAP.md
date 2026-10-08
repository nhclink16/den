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

## Backlog (decided or asked for, not built)
- **Servers follow-ups:**
  - modpack download: the relay rebuilds a Modrinth `.mrpack` when server mods change
  - sleep-when-empty via lazymc
  - a 7 Days to Die server, runnable only while Minecraft is down
  - Spark mod for TPS and CPU (needs a Minecraft restart)
  - the playit join address on the page
- **MW2 1v1s:** a private IW4x server (`sv_lanonly 1` plus a password), a Start 1v1 button on the Servers page, and a hangout card with an `iw4x://` Join link. Waiting on Nicholas owning MW2.
- **GIF search** via Klipy (in progress 2026-10-08). Klipy's rules: every search and GIF load goes straight from the client to Klipy, never through Den, and nothing is copied or re-hosted. Sent GIFs and GIF profile pictures and wallpapers are Klipy links. The search box says "Search KLIPY". Submit the production key form once it is live; the test key allows 100 searches an hour.
- **Landing page at `/`, app at `/app`** (deferred 2026-09-16).
- **Usability pass:** an agent uses Den like a friend would (desktop, web, phone) and ranks the rough spots.
- **Mac code signing:** needs an Apple Developer ID. Until then Mac builds are unsigned and Mac auto-update does not work.
- **RCON hardening:** Minecraft RCON listens on all interfaces with its password in the system unit. Bind it to localhost (needs approval: system unit change).
