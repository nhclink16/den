# Roadmap

Each milestone is shippable on its own. Do them in order.

## M0 — skeleton (done)
Workspace builds, `den-server` serves `/health`, `den health` talks to it, Svelte app scaffolded.

## M1 — text chat, CLI first
- SQLite + migrations: users, sessions, tokens, categories, channels, messages.
- Auth: register with invite code, login, session cookie, bearer tokens for agents.
- REST: channels CRUD (admin), messages list/create/edit/delete, users list.
- WebSocket `/ws` streaming `Event`.
- OpenAPI via utoipa at `/openapi.json`.
- CLI: `den login`, `den channels`, `den send <channel> <text>`, `den tail [channel]`, `den token create --bot <name>`.
- `skills/den/SKILL.md` written against the real CLI.
Done when: two terminals can chat through the CLI and an agent can post with a bot token.

## M2 — web client
- Login, channel list with categories, message view, composer with markdown, replies, reactions, mentions, typing, presence.
- Two collapsible columns, member drawer, Ctrl+K palette, settings page.
- Unread inbox and quiet-by-default notification prefs.
- Uploads: chunked, 1 GB default cap, range-served, inline video/audio players, image thumbnails. Link previews.
- Search (SQLite FTS5).
Done when: the group can use it in a browser instead of Discord for text.

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
- Hetzner box, Caddy, backups, domain.
- OpenClaw channel plugin so Clanker joins.
