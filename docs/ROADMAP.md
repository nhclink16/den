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

## M2 — web client
- Login, channel list with categories, message view, composer with markdown, replies, reactions, mentions, typing, presence.
- Two collapsible columns, member drawer, Ctrl+K palette, settings page.
- Unread inbox and quiet-by-default notification prefs.
- Server APIs for replies, reactions, read state, notification prefs, search (FTS5). (Astra)
- Inline video/audio players, image thumbnails, upload progress. (Fable)
- First real deployment: Hetzner box, domain, Caddy TLS, backups. Friends need a URL, not localhost. (Astra)
- Search (SQLite FTS5).
Done when: the group can use it in a browser at a real URL instead of Discord for text.

## M2.5 — iOS feasibility spike
Tauri 2 build of the text client plus a bare LiveKit audio call on a physical iPhone from the iMac. Test camera and mic permissions, Bluetooth audio routing, interruptions, locked-screen audio, reconnection, and a large upload. This decides Tauri vs SwiftUI for M5 before any call UI is built. (Astra)

## M3 — voice and video
- LiveKit in compose, server mints tokens, hangout room and DM calls.
- Docked cam strip, screen share, device picker, voice activity and PTT.
Done when: a gaming session runs on it with cams.

## M4 — desktop
- Tauri 2 shells for Windows and Linux, global PTT shortcut, native notifications, auto-update.

## M5 — iOS
- Tauri 2 iOS build tested on the iMac. If webview call quality is bad, fall back to a native SwiftUI shell using the LiveKit Swift SDK.
- APNs push.

## M6 — deploy and integrations
- Link previews with an SSRF-safe fetcher.
- OpenClaw channel plugin so Clanker joins.
