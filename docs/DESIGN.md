# Design decisions

Settled 2026-09-08. Change these by editing this file, not by drifting.

## Product
- One instance = one friend group. No multi-server, no federation, no E2EE.
- Roles: admin and member. Nothing finer.
- Text channels grouped in categories, DMs and group DMs.
- One persistent voice room ("hangout") plus ad hoc calls from DMs. Webcams and screen share. Docked cam strip so text stays visible in a call.
- Push-to-talk (Tauri global shortcut) and voice activity, user picks.
- Messages: markdown, replies, reactions, uploads, link previews, mentions, pins, search, typing, presence. Not: threads, forums, stickers, custom emoji.
- Uploads are the reason this project exists. Default cap 1 GB per file, admin-adjustable, no per-user tier. Video and audio play inline, images get thumbnails. Files live on local disk on the VPS, streamed with HTTP range requests so long clips scrub. Disk usage shown on the admin settings page, no automatic deletion.
- Quiet by default: only mentions, DMs, and explicitly subscribed channels notify. Unread inbox view instead of badges everywhere.
- UI: two collapsible columns plus optional member drawer. Command palette (Ctrl+K). Settings is a page with search, not a modal. Density toggle later.
- Auth: username + password, invite codes to register, long-lived sessions.

## Agents
- API tokens are first-class users. A token is either a bot identity (badge, own name) or a token for a human's account (posts as them).
- `den` CLI wraps the REST API and the event stream (`den send`, `den tail`, ...). Skills for Claude Code and Codex wrap the CLI. No MCP server.
- Clanker joins via an OpenClaw channel plugin (`integrations/openclaw`), after the API settles.

## Stack
- Server: Rust, axum, SQLite (sqlx), utoipa for OpenAPI. Single binary.
- Realtime: one WebSocket per client carrying `den_core::Event`.
- Media: LiveKit self-hosted in the same compose file. Server mints room tokens.
- Client: Svelte 5 + Vite SPA, Tauri 2 shells for Windows, Linux, iOS. iOS work happens on the iMac.
- Push: APNs once an Apple developer account exists.
- Hosting: Hetzner Ashburn (~$8/mo), compose with Caddy for TLS, nightly SQLite backup off-box. Domain to be bought later.
