# Deploy brief: Den on the VPS

Owner: Astra (deploy lane). Read `docs/DESIGN.md`, `docs/M3-NOTES.md`, `deploy/README.md`, `AGENTS.md`, and `~/.claude/skills/fleet/references/devices.md` on codexbox first.

Target: the OVH VPS, SSH alias `vps` (user `debian`, passwordless sudo, Debian 13, 4 cores, 8 GB, public IP `135.148.120.197`, Tailscale `100.81.174.46`, KVM console in OVH's dashboard). Hostnames: `den.nicholascaron.com` for the app, `rtc.den.nicholascaron.com` for LiveKit. Both must be DNS-only A records in Cloudflare, never proxied. If either name does not resolve to the VPS yet, stop and say so; do not proceed to certificates. If `~/.config/den/cloudflare.token` exists on codexbox it is a Cloudflare API token scoped to DNS edit on that zone: use it to create or fix the two records, verify with `dig`, and never print it.

Done when: Nicholas opens `https://den.nicholascaron.com` from his phone on cellular, logs in with his existing demo account, sees his existing messages and clips, joins `hangout`, and a second person on a different network hears and sees him. Backups run and a restore has been proven once.

## Build and install
- Build release binaries on codexbox (`cargo build --release -p den-server -p den`) and the web client (`npm --prefix apps/web ci && npm --prefix apps/web run build`), then copy `target/release/den-server`, `target/release/den`, and `apps/web/dist` to the VPS. Building on the VPS is fine too; pick one and document it as `deploy/release.sh` so a redeploy is one command from codexbox.
- Layout on the VPS: binaries in `/opt/den/bin`, web in `/opt/den/web`, data in `/var/lib/den` (SQLite, uploads), config in `/etc/den/den.env` (mode 600, owned by a dedicated `den` system user). The server runs as that user, not root, not `debian`.
- `den-server` as a systemd unit with `Restart=always`, `DEN_BIND=127.0.0.1:7000`, `DEN_ORIGIN=https://den.nicholascaron.com`, and the LiveKit settings. Hardening: `NoNewPrivileges`, `ProtectSystem=strict`, `ReadWritePaths=/var/lib/den`, `PrivateTmp`.

## TLS and routing
- Caddy from the official Debian repo, as a systemd service. `den.nicholascaron.com` reverse-proxies to `127.0.0.1:7000` with WebSocket support (Caddy does this by default) and `request_body max_size 1100MB` so 1 GB uploads pass. `rtc.den.nicholascaron.com` reverse-proxies to LiveKit's signaling port. Let Caddy obtain certificates over HTTP-01 on 80/443.
- LiveKit `v1.9.0` under systemd from the official binary or Docker, your call, pinned. Host networking. `rtc.use_external_ip: true`, UDP range `50000-50200`, TCP `7881`, and built-in TURN on `rtc.den.nicholascaron.com` with TURN/TLS on `5349` and UDP `3478`, using a certificate Caddy issued (share it via Caddy's storage path or a small post-renew hook; document which). Generate a fresh API key and secret, never reuse the dev ones.
- Firewall with `nftables` or `ufw`: allow 22, 80, 443, 3478/udp, 5349/tcp, 7881/tcp, 50000-50200/udp, and everything from the Tailscale interface. Default deny inbound. Keep SSH reachable over Tailscale as the fallback and confirm `ssh vps` still works after the rules apply. Nicholas approved firewall rules on the VPS in this brief; the codexbox firewall stays untouched.
- Add the VPS's `/health` and the LiveKit port check to whatever the fleet skill uses for status, if it has a hook for that; otherwise skip.

## Data
- Migrate the demo: stop `den-demo` on codexbox, copy its SQLite database (use `sqlite3 .backup`, not `cp`) and the uploads directory to the VPS, set ownership to `den`, start the server, and confirm the accounts, rooms, messages, and clips are all there. Leave `den-demo` stopped and disabled afterward but keep its data as a fallback; note the path.
- Nightly backup: a systemd timer on the VPS that does `VACUUM INTO` of the database plus an `rsync` of uploads to codexbox at `/mnt/storage/den/backups/<date>/` over Tailscale, keeping 14 days. Use a dedicated SSH key from the `den` user to a restricted account or a forced command on codexbox; do not reuse Nicholas's keys. Prove restore: on codexbox, start a throwaway `den-server` against a restored copy and hit `/health` and `/channels` with a token.

## Monitoring
- A Hermes cron (Nicholas's agent host) or a plain systemd timer on codexbox that curls `https://den.nicholascaron.com/health` every five minutes and messages Nicholas on Telegram when it fails twice in a row and again when it recovers. Use whatever notification path already exists on this box; do not build a new one.

## Verification
- From codexbox: `curl -I https://den.nicholascaron.com` shows a valid certificate, `/openapi.json` loads, an upload of the 41-second test clip through the CLI succeeds and streams back with a range request.
- From outside the tailnet: run the M3 smoke script against the public URL with the fake-media Chromium on codexbox, and separately confirm TURN works by forcing relay-only ICE in one context (`iceTransportPolicy: 'relay'` via LiveKit's `rtcConfig`).
- Write `docs/M6-NOTES.md`: what runs where, how to redeploy, how to restore, how to rotate the LiveKit keys. Then stop.
