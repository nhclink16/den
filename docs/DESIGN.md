# Design decisions

Settled 2026-09-08. Change these by editing this file, not by drifting.

## Product
- One instance = one friend group. No federation, no E2EE. A client may connect to several independent instances (native shells only, see `docs/HOSTING-PLAN.md`); instances never share identity or data.
- Roles: admin and member. Nothing finer.
- Text channels grouped in categories, DMs and group DMs.
- One persistent voice room ("hangout") plus ad hoc calls from DMs. Webcams and screen share. Docked cam strip so text stays visible in a call.
- A user can join the same call from multiple devices. Each connection can publish its own camera and screen share, with no per-account device cap. Presence counts people once. Additional devices join with microphone and sound off, and clients suppress their own account's remote microphone audio while allowing its screen-share audio. Leaving one device leaves the others connected.
- Push-to-talk (Tauri global shortcut) and voice activity, user picks.
- Messages: markdown, replies, reactions, uploads, link previews, mentions, pins, search, typing, presence. Not: threads, forums, stickers, custom emoji.
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
- Client: Svelte 5 + Vite SPA, Tauri 2 shells for Windows, Linux, iOS. iOS work happens on the iMac.
- Push: APNs once an Apple developer account exists.
- Hosting: OVHcloud VPS, US-East Vint Hill VA, Debian 13, $10/mo monthly (bought 2026-09-12; Hetzner Ashburn had no cheap plans and rejected cards). Public IP 135.148.120.197, host vps-2fd9743a.vps.ovh.us. Originally planned as Hetzner Ashburn (~$8/mo), compose with Caddy for TLS, nightly backup using `VACUUM INTO` or the sqlite3 `.backup` command for a consistent snapshot under WAL, plus uploads, shipped off-box, with a restore drill before friends move over. A reachable TLS deployment is part of M2, so the domain gets bought then.
- Link previews wait until the fetcher is SSRF-safe (deny private ranges, size and time limits).
