# M7b — hosts, grants, and terminals

Completed 2026-09-13 on codexbox. Public app: https://den.nicholascaron.com.

The work landed in order: the persistent host (`6477f0b`, `d6fcd0f`), the grant model and Settings/CLI (`197db1d`, `ab5db24`), then the Ghostty plugin (`a3d67f8`). Subsequent small commits fixed decision atomicity, resize/reattach ordering, replay dimensions, and verification races. Application release `af19ccd` is deployed; `13bc761` updates the smoke script. All changes were pushed to `main` with explicit-path staging.

## Use it

Codexbox is already enrolled to **nicholas on the public server**. Its user systemd service runs `/home/nicholas/.local/bin/den-host run`; `den-host status` reports `connected`. The default host configuration now targets production, not the dev server.

In Den, open **Just you** and send `/terminal codexbox`. Run commands in the terminal. **Post card** shares the same session into another room. Another member asks for access; the owner allows the request in their DM. A control grant lets the member request the active controller slot. The owner gives control from the terminal view or call tile, then can **Take back** or **Revoke control**. Take back retains the grant; revoke removes it.

For a fresh Linux or macOS machine, use **Settings → Machines → Add a machine** and copy its one-time code into the displayed command:

```sh
cargo install --git https://github.com/nhclink16/den --locked den-host && den-host login '<code from Settings>' && den-host install
```

Rust/Cargo must be installed and its bin directory on PATH. The code includes the server origin and expires after ten minutes. Run as the logged-in user. On Linux the installer enables and restarts the user service; configuration is `~/.config/den/host.toml`, verified mode `600` on codexbox. `journalctl --user -u den-host` shows connection and viewer notifications. Unenroll through **Remove** in Machines to invalidate the host token and terminate sessions.

macOS installation is implemented but **untested**: run the same commands, then inspect `~/Library/LaunchAgents/com.den.host.plist` and `launchctl print gui/$(id -u)/com.den.host`. Windows installation is implemented but **untested**: run `cargo install`, `den-host login`, and `den-host install` as separate commands in PowerShell, then inspect the **Den Host** logon Scheduled Task with `schtasks /Query /TN "Den Host" /V`. Windows uses the user's profile for `.config/den` and portable-pty's ConPTY support. These are platform follow-up checks, not claimed acceptance results.

## Contract and implementation

- Shared request, state, event, and host protocol types live in `den-core`; OpenAPI and the generated web schema include them. Migration `0005_hosts.sql` adds hosts, enrollment codes, grants, audit records, session/card mappings, and protected recording mappings.
- The host dials `/hosts/ws` with its own bearer credential. `HostFrame` is UTF-8 JSON carried in binary WebSocket messages; byte fields are numeric octet arrays. Output coalesces at 16 ms, with a 1 MiB unacknowledged window and 256 KiB recent history per PTY. `HostTransport` and `ScreenSource` are the seams for later transports and screen sources.
- PTYs survive a host WebSocket disconnect while the host process remains running. Reattachment requests recent host scrollback plus a resize nudge; the server does not retain an output replay buffer. Restarting the host process terminates its PTYs. Viewers other than the owner produce a log line and a best-effort Linux desktop notification.
- A terminal card's room visibility does not grant PTY access. Viewing requires host ownership or an active view/control grant, and visibility of a card for the session. Only the active controller can type. The owner chooses that controller; a control grant alone does not take over an existing session.
- Access decisions write the grant, updated request card, and audit records in one transaction. The integration test injects an audit failure and verifies the grant rolls back. Expiry and revocation are checked on input; viewer subscriptions are rechecked every 500 ms. Audit history shows the latest 200 entries in Settings, Access.
- The direct shortcut binds only the host's Tailscale interface on a random port. On codexbox it uses WSS with a Tailscale certificate, making it usable from the HTTPS app. Without a reachable direct listener, the browser falls back after 700 ms to the existing Den socket. Tickets last 60 seconds and refresh at 50 seconds. The host checks view permission every 500 ms and checks each input against Den, with a 450 ms HTTP timeout. Output still goes to Den for recording.
- Recordings are protected uploads containing newline-delimited `[ms, base64]` values. Each encoded chunk begins with an ANSI `CSI 8;rows;cols t` dimension marker, followed by the actual output bytes. Replay applies those dimensions before writing to Ghostty. The file caps at 64 MiB, with `recording_capped` in session state. Recording access remains guarded after card/host deletion through its retained mapping.
- The terminal plugin pins `ghostty-web` **0.4.0**, loads it lazily, and bundles IBM Plex Mono with its license. Herdr rendered successfully; no renderer substitution was made. Dock, expanded view, and call tile share the renderer. Small viewer/tile surfaces scale the controller's grid; the controller's main dock resizes the PTY. The preview is local recent screen text, not a new server screen-state protocol.
- Focused terminals receive Den/call shortcuts. Double Escape within 400 ms releases focus. Ghostty 0.4.0's custom handler consumes a key when it returns `true`. Replay scrubbing pauses playback and uses ANSI RIS plus a canvas clear to avoid stale pixels from resetting the live buffer.

The CLI provides `den host list`, `den host enroll`, `den access request`, `den access grants`, `den access revoke`, `den terminal open`, and `den terminal write`. The Machines and terminals section in `skills/den/SKILL.md` documents the ask/wait/act flow and a minimum five-second interval for agent polling.

## Verification

Passed locally:

```sh
cargo test --locked --workspace --target-dir /mnt/storage/den-m7b-target
npm --prefix apps/web run check
npm --prefix apps/web run build
DEN_SMOKE_HOST_BIN=/mnt/storage/den-m7b-target/debug/den-host node scripts/m7b-smoke.mjs
```

The workspace run passed 19 server integration tests and two host integration tests. The host tests exercise a real PTY, input, resize, shell-state retention, exactly 1 MiB of backpressure, resumption after Ack, exit status, enrollment configuration, and socket reconnection. Web checks reported zero errors and warnings; production builds succeeded. The existing large-chunk Vite warning remains.

Dev smoke passed over direct and forced-relay paths. The dev API/Vite/LiveKit setup remains the one described in M3/M6/M7a notes. On this machine, build artifacts were moved to `/mnt/storage/den-m7b-target` because the root filesystem lacked space for duplicate Rust builds; `deploy/release.sh` now respects `CARGO_TARGET_DIR` and builds `den-host` alongside server and CLI.

Before deployment, `den-backup` completed successfully on the verified VPS `vps-2fd9743a`. Then:

```sh
CARGO_TARGET_DIR=/mnt/storage/den-m7b-target ./deploy/release.sh
DEN_SMOKE_URL=https://den.nicholascaron.com \
DEN_SMOKE_CREDENTIALS="$HOME/.local/share/den-m6/smoke-credentials.json" \
DEN_SMOKE_HOST_BIN="$HOME/.local/bin/den-host" \
node scripts/m7b-smoke.mjs
```

The final public run passed with **direct** transport:

```text
Owner keyboard -> PTY -> Ghostty passed (direct)
View request, control request, owner promotion and Bob input passed
Revoked browser and forged API input both denied
PASS https://den.nicholascaron.com: session 01M2E9NJ1A08BTT340KRXQQ8VR,
shared card 01M2E9NK2MJ90R21VBWXRPVAWJ
```

A public run with `DEN_SMOKE_RELAY=1` also passed, session `01M2E9G11GSQXTERNEEVNRYZDF`, shared card `01M2E9G2F20SGB1BSTKRZ46R2W`. That test blocks direct Tailscale sockets in Playwright and asserts the relay is used. It ran before the final card-only CSS adjustment; transport code is unchanged. Its screenshots are retained locally under `~/.local/share/den-m7b/public-relay/`.

Both browser contexts use real keyboard input and inspect Ghostty's actual terminal buffer. The smoke checks DEN_OK, shared scrollback, automatic attachment after approval, giving control from the call tile, BOB_OK on both screens, revoke within one second, two seconds without further Bob input, and HTTP 403 for forged input. It runs `herdr --session den-m7b-smoke`, types a command there, and drives SSH from that terminal to obtain `vps-2fd9743a`. It checks resizing, keyboard focus, joining a real LiveKit call with fake media devices, ending the session, full Herdr/VPS replay, and seeking back to BOB_OK. It waits for the recording download before seeking.

The smoke resets only m6_bob's grants on its selected smoke host and closes the session it creates. It can enroll and switch the local host to the selected environment. Tests leave their chat cards and recordings for inspection. Browser contexts are automated Chromium at 1440×900 and 390×844; physical mobile keyboards and native clients were not tested.

After release, public health returned `ok: true`. SQLite integrity was `ok`, foreign-key violations were zero, and migrations 1–5 all reported success. The Linux service remained active with the production credential stored mode 600.

## Reviewed screenshots

All fourteen public screenshots were opened and reviewed. The request screenshots show decision history on desktop and a pending Allow/Deny card on mobile.

| Surface | Desktop | Mobile |
| --- | --- | --- |
| Terminal card | [desktop](shots/m7b-card-desktop.png) | [mobile](shots/m7b-card-mobile.png) |
| Access requests | [desktop](shots/m7b-access-request-desktop.png) | [mobile](shots/m7b-access-request-mobile.png) |
| Control banner | [desktop](shots/m7b-control-banner-desktop.png) | [mobile](shots/m7b-control-banner-mobile.png) |
| Docked Herdr | [desktop](shots/m7b-docked-herdr-desktop.png) | [mobile](shots/m7b-docked-herdr-mobile.png) |
| Expanded Herdr | [desktop](shots/m7b-expanded-herdr-desktop.png) | [mobile](shots/m7b-expanded-herdr-mobile.png) |
| Call tile | [desktop](shots/m7b-call-tile-desktop.png) | [mobile](shots/m7b-call-tile-mobile.png) |
| Replay | [desktop](shots/m7b-replay-desktop.png) | [mobile](shots/m7b-replay-mobile.png) |

M7b stops here. File transfer, screen control, direct WebRTC, screen-state synchronization, and native mobile terminals remain outside this milestone.
