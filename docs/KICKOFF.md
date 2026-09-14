# Kickoff brief for Astra

Written by Fable (Claude) 2026-09-08. You are the second engineer on Den. Nicholas owns the product, we own the build.

## Why this exists
Nicholas tried to post a 41-second clip in Discord and hit the upload cap. That, plus Discord's three-column sprawl, loud notifications, and the pain of registering bots, is the whole motivation. Den is a self-hosted chat for five friends who game with cams on, and who want their AI agents in the chat with zero ceremony.

Read in this order: `README.md`, `docs/DESIGN.md`, `docs/ROADMAP.md`, `AGENTS.md`, then skim the three crates and `deploy/`.

## What I want from you first
Nitpick the decisions in DESIGN.md and the skeleton before we write real code. Push back hard where you disagree, and say so plainly when you agree. Specific things I am least sure about:

1. SQLite via sqlx vs rusqlite. I lean sqlx for async and compile-checked queries, but rusqlite plus a blocking pool is simpler and SQLite is single-writer anyway.
2. utoipa for OpenAPI. Worth it, or does it add ceremony for a five-user app whose only API consumers are our own CLI and client?
3. One WebSocket carrying `den_core::Event` for the client, and the CLI `tail` using the same stream. Alternative is SSE, which is simpler for the CLI. Pick one.
4. Chunked uploads to local disk with range serving, 1 GB default cap. Anything smarter needed for iOS Safari or Tauri webviews to play long clips inline?
5. Tauri 2 on iOS for the first attempt vs native SwiftUI shell from day one. You will be doing the iOS work on the iMac, so your call weighs more than mine.
6. LiveKit self-hosted in compose with `network_mode: host`. Is that the right layout next to Caddy, or should Caddy sit in front of LiveKit too?
7. Auth: password + invite code, long sessions, bearer tokens for agents. Anything you would change?
8. Anything in `AGENTS.md` you think is wrong or missing. It is also the Codex instructions file.

## Proposed work split
Nicholas's preference: Fable owns UI/UX, Astra owns most everything else.

Astra:
- M1 server: migrations, auth, channels, messages, WebSocket, OpenAPI, uploads.
- M1 CLI: `den login|channels|send|read|tail|token`.
- M3 media: LiveKit config, token minting, room lifecycle.
- M5 iOS build and testing on the iMac.
- M6 deploy: Hetzner, Caddy, backups, and the OpenClaw channel plugin.

Fable:
- M2 web client: all Svelte UI, layout, palette, settings page, inbox, composer, media players.
- M3 call UI: docked cam strip, screen share picker, device settings, PTT.
- M4 Tauri desktop shells, global shortcut, native notifications.
- Skills for Claude Code and Codex, docs.

Shared contract: `den-core` types and the OpenAPI spec. Whoever needs a type change edits `den-core` and tells the other. I will not build UI against an endpoint that does not exist yet; you will not change an endpoint the client already uses without a note.

Sequencing: you start M1 now. I start M2 against mocked data from `den-core` and switch to the real server when your M1 lands. We each commit to `main` directly with small commits; branches only for anything risky.

## How to answer
Write your review to `docs/REVIEW-astra.md`: numbered responses to the eight questions, any other objections, and your counter-proposal on the split if you have one. Keep it under 800 words. Then reply in the terminal with a three-line summary. Do not start building yet.
