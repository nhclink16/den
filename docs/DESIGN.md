# Design decisions

Settled 2026-09-08. Change these by editing this file, not by drifting.

## Product
- One instance = one friend group. No federation, no E2EE. A client may connect to several independent instances (native shells only, see `docs/HOSTING-PLAN.md`); instances never share identity or data.
- Roles: admin and member. Nothing finer.
- Text channels grouped in categories, DMs and group DMs.
- One persistent voice room ("hangout") plus ad hoc calls from DMs. Webcams and screen share. Docked cam strip so text stays visible in a call.
- A user can join the same call from multiple devices. Each connection can publish its own camera and screen share, with no per-account device cap. Presence counts people once. Additional devices join with microphone and sound off, and clients suppress their own account's remote microphone audio while allowing its screen-share audio. Leaving one device leaves the others connected.
- Push-to-talk (native desktop keyboard hook) and voice activity, user picks.
- Messages: markdown, replies, reactions, uploads, link previews, mentions, pins, search, typing, presence. Not: forums, stickers. Threads and custom emoji were excluded here and later adopted; see `DIRECTION.md`.
- Uploads are the reason this project exists. Default cap 1 GB per file, admin-adjustable, no per-user tier. Video and audio play inline, images get thumbnails. Files live on local disk on the VPS. Uploads are chunked and resumable, finalized atomically, abandoned temp files cleaned. Serving is authenticated and supports HEAD and 206 range responses. No transcoding to start; H.264/AAC MP4 with `playsinline` is the tested path, HLS deferred until a real clip fails. Disk usage shown on the admin settings page, no automatic deletion.
- Quiet by default: only mentions, DMs, and explicitly subscribed channels notify. Unread inbox view instead of badges everywhere.
- UI: two collapsible columns plus optional member drawer. Command palette (Ctrl+K). Settings is a page with search, not a modal. Density toggle later.
- Auth: username + password (Argon2id, login throttling), expiring use-limited invite codes, first admin bootstrapped from the CLI on an empty database. Opaque revocable sessions in Secure/HttpOnly/SameSite cookies, CSRF protection on cookie-authenticated writes, WebSocket origin check.

## Agents
- Bots are users with `bot = true`, a human owner, and member permissions by default. API tokens belong to a user, human or bot; a human's token posts as that human. Tokens are random, stored hashed, shown once, revocable.
- `den` CLI wraps the REST API and the event stream (`den send`, `den tail`, ...). Skills for Claude Code and Codex wrap the CLI. No MCP server.
- Agent hosts join through platform plugins, after the API settles. Hermes first (`integrations/hermes`, out-of-tree plugin via `BasePlatformAdapter`, zero core changes). OpenClaw second and optional (`integrations/openclaw`).

## Stack
- Server: Rust, axum, SQLite via sqlx (WAL, foreign keys, busy timeout, small pool, query macros with committed `.sqlx` metadata). utoipa for OpenAPI; schema derives live in `den-core`, HTTP annotations in the server. TypeScript client types are generated from the spec. Single binary that also serves the SPA.
- IDs are ULIDs, so they sort by creation time. Pagination is by id.
- Realtime: one WebSocket per client carrying `den_core::Event`, used by both the web client and `den tail`. Heartbeat, reconnect with backoff, and a resync-by-refetch after a gap; no durable event replay. Events are filtered by channel membership. `tail` emits JSON lines and reports gaps.
- Media: LiveKit self-hosted, pinned version, host networking. Signaling goes through Caddy on `rtc.<domain>`, WebRTC media ports exposed directly, built-in TURN enabled for restrictive networks, with TURN/TLS on 443 of the `rtc` host and its own cert, wired up at deploy time in M2. Server mints room tokens; membership is checked before minting.
- Client: Svelte 5 + Vite SPA, Electron shells for Windows, Linux, macOS. The bundled Chromium engine provides WebRTC on Linux; the Tauri bridge remains compatible during migration. iOS is a native SwiftUI app (decided 2026-09-14, no webview shell), built on the iMac.
- Push: APNs from the server, key at ~/.config/den/apns.p8 on codexbox; Apple developer account exists since 2026-09-12.
- Hosting: https://denchat.app on the OVHcloud VPS in US-East Vint Hill, VA, Debian 13. Public IPv4 135.148.120.197. Native Den and LiveKit services, Caddy HTTPS and HAProxy routing for TURN/TLS on rtc.denchat.app:443. Nightly offline exports go to codexbox with 14-day retention and real-server restore drills.
- Link previews wait until the fetcher is SSRF-safe (deny private ranges, size and time limits).

## WebSocket compatibility

The `/ws` notification stream can gain new event types. Clients must ignore unknown
types and continue receiving known events on the same connection. Missing or invalid
fields in a known event remain errors. Rust's `Event::Unknown` is a receive-only
fallback, excluded from serialization and the generated API schema. `den tail`
skips it; web and native iOS already dispatch by tag and ignore unrecognized tags.
`den-host` uses the separate `HostFrame` protocol, which this rule does not change.

Keep the existing `music` and `sounds` query gates. New event types also need gates
while any supported `/ws` consumer rejects unknown types. Landing the catch-all
only protects clients built with it; it cannot repair an installed older binary.

New notification types can be ungated, and the existing gates can be removed in a
separate change, only after a recorded compatibility review establishes all of:

- Every supported `/ws` consumer has been identified, including running agent
  processes, scheduled jobs, and clients on temporarily offline machines.
- Each consumer is verified to ignore an unknown event and receive a following
  known event, or has been upgraded or retired. Rebuilds alone are insufficient;
  running processes must use the compatible build.
- Support for incompatible builds has explicitly ended. Supported installers and
  rollback procedures must not restore them against the newer server.

Until that review is recorded, the gates stay and their legacy-stream tests remain.
There is no calendar-based removal deadline.
