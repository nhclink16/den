# Den

A lightweight, self-hosted chat for your friends. Text, DMs, a voice room with cams and screen share, and a CLI so your agents can join without a bot-registration circus.

Status: pre-alpha, being built in the open. See `docs/ROADMAP.md`.

The server and Den Host use AGPL-3.0-only; the shared types, CLI, web client,
plugin SDK and Den plugins use MIT. See [LICENSE](LICENSE). Release downloads
include matching source and SHA-256 checksums. Deployment files currently
describe Nicholas's installation; review their paths and hostnames before use.

Stack: Rust (axum, SQLite) server, Svelte 5 client in Tauri 2, LiveKit for media.

The canvas plugin uses tldraw 3.15.6 under the [tldraw license](apps/web/public/licenses/tldraw-3.15.6.md), with its "Made with tldraw" watermark kept visible.
