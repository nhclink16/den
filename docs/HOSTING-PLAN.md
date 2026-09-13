# Hosting, licensing, and multi-server plan (draft for review)

Written by Fable 2026-09-14 for Astra to review. Scope stays "software for Nicholas and his friends" for now. Renting bigger servers, payments, and a hosted product come later. This document decides what we build now so that later is cheap, and asks Astra to disagree where the seams are wrong.

## Positions to review

1. **License split.** Server (`den-server`, `den-host`) under AGPL-3.0. `den-core`, the CLI, the web client, the plugin SDK and all plugins under MIT. Effect: anyone can self-host free; anyone hosting a modified server must publish changes; plugins can be any license, including paid. Question for Astra: is the web client better under AGPL too, given the client is where most of the product is, and does MIT on the client weaken the fork protection meaningfully?

2. **Hosted model, later.** Multi-tenant on our own boxes: one `den-server` process per tenant, own SQLite and uploads dir, own subdomain, shared LiveKit per box with per-tenant API keys, Caddy wildcard cert via the Cloudflare DNS token, Stripe Checkout and webhooks, a `den-cloud` crate for provisioning. Not built now. Question: what in today's server would make this painful, and what tiny changes now prevent that? Candidates: config through one env file already, but LiveKit key per tenant, listener port per tenant, and the absence of a "instance name" concept.

3. **Build now, because self-hosters want it anyway and it is the trust feature for hosting.**
   - **Export and import.** `GET /admin/export` streams a zip: `den.db` via `VACUUM INTO`, `uploads/`, and a `manifest.json` with schema version and instance name. `den admin export` and `den admin import <zip>` in the CLI; import runs only against an empty database and refuses on schema mismatch newer than the binary. Settings, Account gets "Download everything" for admins.
   - **Instance identity.** A `settings.instance_name` (default "Den") and `instance_icon_upload_id`, editable by admins in Settings, Rooms. The client shows the name where it shows "Den" today. Needed for multi-server and for the hosted product, cheap now.
   - **Prebuilt binaries.** A GitHub Actions release workflow that builds `den-server`, `den`, and `den-host` for linux-x86_64, linux-aarch64, macos-aarch64, macos-x86_64, windows-x86_64, and attaches them to a tagged release, plus an `install.sh` for the host that downloads the right one. Friends must not need Rust.
   - **Game server plugin.** Deferred. Note the shape only: `/server minecraft` on a host, itzg/minecraft-server container via den-host, status card, console tile. No code now.

## Multi-server client (design to review)

A "server" is a separate Den instance with its own accounts, the way Slack workspaces or Mastodon instances work. The client holds several sessions at once. Requirements from Nicholas: no Discord-style icon rail, compact, no sidebar clutter.

Proposed design:
- **The wordmark is the switcher.** The top-left "Den" in the sidebar becomes the current instance's name with a small chevron. Clicking opens a popover listing connected instances with their icon, name, unread count, and a lit dot for mentions, plus "Add a server" at the bottom. Keyboard: `Ctrl+1..9` switches, `Ctrl+Shift+]` cycles. Nothing else changes in the sidebar; it always shows exactly one instance's rooms.
- **Cross-instance signals without a rail.** The switcher chevron carries an amber dot when another instance has a mention or DM. The Inbox merges all instances, grouped by instance with an eyebrow. Ctrl+K searches rooms and people across every connected instance, results prefixed with the instance name in mono.
- **Adding a server.** Paste a URL or an invite link. Login or registration happens against that instance. Sessions are per instance; logging out of one does not touch the others.
- **Calls and docks are per instance.** You can be in one call at a time. Switching instances while in a call keeps the call dock visible with the instance name in it, so you always know which server you are talking in.
- **Storage.** `localStorage` keys become namespaced per instance origin. The store becomes a map of instance stores with one "active"; the WebSocket per instance stays open in the background at low priority (no message bodies fetched until active, only read-state and presence events applied).
- **Not doing.** No federation, no shared identity across instances, no cross-instance DMs. Each instance is its own world; the client is just a window with tabs.

Alternative considered: a compact horizontal strip of instance icons above the room list, shown only when more than one instance is connected. Rejected as a first choice because it is Discord's rail turned sideways, but Astra should say if the popover fails at, say, four instances.

## Ask
Write `docs/REVIEW-hosting.md`: numbered responses to the three positions and the multi-server design, objections, and a concrete proposal for the build order of the "build now" items with estimates in hours. Under 700 words. Do not build anything.

## Agreed 2026-09-14 (after `REVIEW-hosting.md`)

1. **License.** `AGPL-3.0-only` for `den-server` and `den-host`. MIT for `den-core`, the CLI, the web client, the plugin SDK and plugins. AGPL is not fork protection; it obliges hosted modifications to publish source to their users, which is what we want. The client stays MIT partly because tldraw's license would complicate an AGPL client. Apply the split at the open-source release, not before.
2. **Hosted model.** Deferred entirely. When it comes: one process and data dir per tenant, **one LiveKit per tenant** (API keys do not partition rooms; a shared instance is a cross-tenant risk), resource isolation per tenant, and M6's Caddy/HAProxy/TURN path kept. No Stripe, no `den-cloud` crate now.
3. **Export/import.** Offline operator CLI first: `den-server export <zip>` and `den-server import <zip>` run with the server stopped, include partial uploads and recordings, exclude deployment secrets, carry a manifest with schema version, migration checksums, and file hashes. Import goes into an unused data dir, validates paths, integrity, and foreign keys, migrates older schemas, rejects newer ones, and invalidates sessions, tokens, invites, host credentials and grants by default (hosts re-enroll). An admin-facing HTTP download comes later, named "Export server backup", after a privacy review, because it contains every DM and hash.
4. **Instance name.** Bounded `settings.instance_name` via `den-core` and a migration, shown where the client says "Den". Icon deferred until upload visibility is independent of rooms.
5. **Prebuilt binaries first.** Release workflow producing linux-x86_64 (server bundle with SPA, CLI, host), macOS arm64 and Windows x86_64 (CLI and host), with checksums and a host `install.sh`. Friends must not need Rust. Real enrollment checks on each platform are the acceptance.
6. **Multi-server client.** Design stands: the wordmark is the switcher, a popover with icon, name, unread count and a textual mention indicator, origins shown when names collide, keyboard focusable; merged Inbox and cross-instance palette. Shortcuts avoid the browser's `Ctrl+1..9`. **It ships in the native shells only** (Tauri, M4): the browser client cannot authenticate to a second origin with cookie sessions, and we are not weakening origin checks for it. Native shells use bearer sessions in OS credential storage; the server needs a WebSocket token parameter for that. In the browser, one instance per tab. Calls stay attached to their originating instance; logout disconnects it.
7. **Game servers.** Deferred. Docker control on a host is a separate capability from terminal access.

Build order and estimates from the review: binaries 16–24 h, offline export/import 20–32 h, instance name 3–5 h alongside, admin export later 6–10 h. These become M8 in the roadmap.
