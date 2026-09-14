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
Retired 2026-09-14. Decision: the iPhone app is native SwiftUI, not a webview. See M5.

## M3 — voice and video
- LiveKit in compose, server mints tokens, hangout room and DM calls.
- Docked cam strip, screen share, device picker, voice activity and PTT.
Done when: a gaming session runs on it with cams.

## M4 — desktop (started 2026-09-14, brief in `docs/M4-DESKTOP-BRIEF.md`)
- One Tauri 2 project for macOS, Windows, Linux: native sessions in the keychain, multi-server switcher, global PTT, native notifications, tray, updater, signed releases.

## M5 — iOS, native SwiftUI (decided 2026-09-14)
Native app, iOS 26 and Liquid Glass materials, LiveKit Swift SDK for calls. Canvas and terminal tiles embed a web view inside native tiles. Built on the iMac by Astra.
- M5a text: login, rooms, messages, DMs, inbox, uploads and video, settings, APNs push (server gains push sending). Started 2026-09-14, brief in `docs/M5A-IOS-BRIEF.md`.
- M5b calls: LiveKit Swift, CallKit ringing for DM calls, background audio, picture-in-picture, phone-sized layouts with every share visible.
- M5c: plugin tiles in embedded web views, multi-server switcher, QR pairing to add a server.

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
