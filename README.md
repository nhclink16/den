# Den

A lightweight, self-hosted chat for your friends. Text, DMs, a voice room with cams and screen share, and a CLI so your agents can join without a bot-registration circus.

The live app is https://denchat.app.

Status: pre-alpha, being built in the open. See `docs/ROADMAP.md`.

The server and Den Host use AGPL-3.0-only; the shared types, CLI, web client,
plugin SDK and Den plugins use MIT. See [LICENSE](LICENSE). Release downloads
include matching source and SHA-256 checksums. Deployment files currently
describe Nicholas's installation; review their paths and hostnames before use.

Stack: Rust (axum, SQLite) server, Svelte 5 client in Tauri 2, LiveKit for media.

The canvas plugin uses tldraw 3.15.6 under the [tldraw license](apps/web/public/licenses/tldraw-3.15.6.md), with its "Made with tldraw" watermark kept visible.

Download native server, CLI and host binaries from [Releases](https://github.com/nhclink16/den/releases).
Connect a machine from Settings → Machines; the generated installer command
checks its download and enrolls the host. Server owners can change the displayed
name in Settings → Rooms.

For offline backups, stop the server and run `den-server export backup.zip` with
`DEN_DB` and `DEN_UPLOADS` set to its data paths. Restore with
`den-server import backup.zip --into /path/to/empty-data-dir`. Restore invalidates
access credentials by default; use `--keep-credentials` only for disaster recovery.
Archives contain private data. See [M8 notes](docs/M8-NOTES.md) for the format,
limits, backup setup and tested recovery procedure.
