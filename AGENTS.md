# Den — agent instructions

Den is a self-hosted chat app for a small friend group: text channels, DMs, one persistent voice room with cams and screen share, and an API built for agents. Read `docs/DESIGN.md` for the decisions and `docs/ROADMAP.md` for what to build next.

## Layout

- `crates/den-core` shared API types. No framework deps. Server and CLI both use it.
- `crates/den-server` axum + SQLite. REST, one WebSocket event stream, LiveKit token minting.
- `crates/den-cli` the `den` binary. Thin wrapper over the REST API. This is what scripts and agents use.
- `apps/web` Svelte 5 + Vite SPA. Wrapped by Tauri 2 for Windows, Linux, iOS.
- `skills/den` Claude Code skill and Codex plugin that teach an agent to use the CLI.
- `integrations/openclaw` OpenClaw channel plugin so Clanker can join. Built after the API settles.
- `deploy` compose file, Caddy, LiveKit config for the VPS.

## Run

```
cargo run -p den-server            # http://127.0.0.1:7000
cargo run -p den -- health
cd apps/web && npm run dev
```

## Rules

- Keep it simple. Five users. No abstractions for scale that does not exist. No plugin systems, no feature flags, no config for things that have one sane value.
- Write the tests a careful human would write: one integration test per API behavior that matters, a unit test where logic is tricky. Do not write tests for getters, serialization of plain structs, or things the type system already guarantees. A PR with more test code than feature code is a smell.
- Shared types go in `den-core` first. The server, CLI, and OpenAPI spec derive from them. Never hand-duplicate a type in the client.
- Prefer one file over a module tree until a file passes about 400 lines.
- SQLite is the database. Migrations are plain SQL files in `crates/den-server/migrations`, numbered.
- Errors: `anyhow` in binaries, `thiserror` in libraries. Do not build custom error hierarchies.
- No new dependencies for things under 50 lines.
- Commit messages: imperative, one line, body only when the why is not obvious.
