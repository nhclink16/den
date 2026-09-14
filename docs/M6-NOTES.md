# M6 public deployment

Deployed on 2026-09-13 to https://den.nicholascaron.com.
The automated verification section of `M6-DEPLOY-BRIEF.md` passes. Nicholas
tried the phone flow and reported that it looks promising. His detailed results
and a second person on another network confirming audio and video remain
pending. Do not treat the human two-person acceptance check as completed.

## What runs where

The OVH VPS is `vps-2fd9743a`, public IPv4 `135.148.120.197`, Tailscale
`100.81.174.46`, SSH alias `vps`, user `debian` with passwordless sudo.
Both DNS-only A records resolve to that public IPv4 through `dig @1.1.1.1`.
No Cloudflare token changes were needed.

| Component | Service and files |
| --- | --- |
| Den | `den-server.service`, dedicated `den` user; `/opt/den/bin/den-server`, `/opt/den/bin/den`, `/opt/den/web`; loopback `127.0.0.1:7000` |
| Data | `/var/lib/den/den.db`, `/var/lib/den/uploads`, owned by `den` |
| App configuration | `/etc/den/den.env`, mode 600, owned by `den`; public origin and fresh LiveKit API credentials |
| LiveKit | Official checksum-verified `v1.9.0` Linux amd64 binary, `livekit.service`, dedicated `livekit` user; `/etc/livekit/livekit.yaml`, mode 600 |
| HTTPS | Official Caddy Debian repository package, `caddy.service`, `/etc/caddy/Caddyfile`; HTTP-01 on 80 and HTTPS behind HAProxy on 8443 |
| TLS routing | Debian HAProxy, `haproxy.service`, `/etc/haproxy/haproxy.cfg`; public 443 routes HTTP to Caddy and TURN/TLS to LiveKit |
| Firewall | `/etc/nftables.conf`, enabled `nftables.service`; rules live in the separate `inet den_filter` table |
| Backup | VPS `den-backup.timer` and `den-backup.service`; codexbox `den-backup-prune.timer` |
| Monitoring | Codexbox user `den-monitor.timer` and `den-monitor.service`; existing Hermes Telegram delivery |

Den uses `Restart=always`, `NoNewPrivileges`, `ProtectSystem=strict`,
`ReadWritePaths=/var/lib/den`, and `PrivateTmp`. Its process is not root or debian.

LiveKit runs natively with host networking, `rtc.use_external_ip: true`, TCP
7881, UDP 50000-50200, TURN UDP 3478, and TURN/TLS 5349. Signaling is
`wss://rtc.den.nicholascaron.com`. The firewall admits the brief's ports,
established connections, loopback, ICMP, DHCP replies, and all traffic from
`tailscale0`; other inbound traffic is denied. It preserves Tailscale's own
firewall tables. Codexbox's firewall was not changed. Fresh SSH over Tailscale
passed after applying the rules.

### TURN/TLS and certificates

LiveKit 1.9.0 [hardcodes the advertised built-in TURN/TLS port to 443](https://github.com/livekit/livekit/blob/v1.9.0/pkg/service/roommanager.go#L935-L936),
regardless of `turn.tls_port`. A listener on 5349 alone passed a certificate
handshake but failed the actual TLS-only ICE test.

`deploy/haproxy.cfg` fixes that without modifying the pinned LiveKit binary.
It passes HTTP TLS to Caddy at 127.0.0.1:8443. TLS for the RTC hostname without
HTTP ALPN goes to LiveKit at 127.0.0.1:5349. Public clients still use 443;
5349 also remains directly reachable as required. Caddy still terminates app
and signaling HTTPS and handles HTTP-01 issuance on 80. Its `https_port` is an
[internal routing setting](https://caddyserver.com/docs/caddyfile/options#https-port).

Caddy's certificate store is
`/var/lib/caddy/.local/share/caddy/certificates/`. The root-owned
`den-turn-cert.timer` checks every five minutes. `sync-turn-cert.sh` finds the
RTC certificate, validates its expiry and matching public key, copies changed
files to `/etc/livekit/certs/turn.crt` and `turn.key` as root:livekit mode 640,
and restarts LiveKit only when the files change. A renewal can briefly interrupt
calls. An unchanged-certificate check also passed.

LiveKit logging is set to `warn`: this pinned release logs temporary TURN
credentials at info level. Production API credentials were generated on the
VPS and never copied from the dev configuration.

## Redeploy and initial provisioning

From the reviewed Den checkout on codexbox:

```bash
./deploy/release.sh
```

That builds `den-server` and `den` with Cargo's locked release build, runs
`npm ci` and the web production build, transfers binaries and the SPA, installs
them into `/opt/den`, restarts Den, and checks public health. Server binary
replacement is atomic. The web directory is synchronized during deployment,
so open tabs may need a reload. This does not copy the demo database again.

Initial provisioning is recorded in `deploy/install-vps.sh`. To provision the
same VPS from scratch, copy `deploy/` to `/home/debian/den-deploy/` and run
`sudo bash /home/debian/den-deploy/install-vps.sh` there. The script verifies both
DNS names before certificate setup, installs packages and service units,
generates credentials only when absent, and installs the firewall. Install or
restore the database before starting Den; after Caddy issues its certificate,
run `sudo systemctl start den-turn-cert livekit`. Then run the release script.

The source files under `deploy/systemd/` describe the backup and monitoring
units too. Install VPS units in `/etc/systemd/system`; put `backup.sh` in
`/opt/den/bin`. On codexbox, put the prune script in `/usr/local/lib/den` and its
units in `/etc/systemd/system`; monitor units live in
`~/.config/systemd/user`. Reload systemd and enable the corresponding timers.
Codexbox already has user lingering enabled, so monitoring survives logout.

`compose.yml` is the older Docker outline, not the running deployment. It keeps
its separate `Caddyfile.compose`. The existing tailnet LiveKit dev files remain
at `~/.config/den/livekit.env` and `livekit.yaml`.

## Migrated data and onboarding

Stopped the codexbox user service `den-demo`, took a consistent SQLite
`.backup`, then transferred that database and the uploads directory. Before
allowing new activity, every table count matched, integrity was `ok`, and the
foreign-key check was empty. The source contained one account, `nicholas`,
five channels, two messages, one reaction, and zero uploads or upload files.
There were no existing clips in this demo to migrate.

Nicholas's existing password successfully logged in at the public URL. The
original messages appeared in the public browser. `den-demo` is stopped and
disabled; `~/.local/share/den-demo` remains untouched as the fallback. The
consistent migration snapshot and row counts are in
`~/.local/share/den-m6/demo-migration.db` and `demo-counts.json` on codexbox.
Do not restart the old demo as a second writable production instance.

For friends, open Settings → Invites and create an invite with the desired
expiry and number of uses. Copy its link and send it. The link opens registration
with the invite prefilled. A friend chooses a username and a password of at
least 12 characters, clicks Join, then opens hangout and allows microphone and
camera access. They need no Tailscale or app installation.

Two member accounts, `m6_bob` and `m6_ari`, were created for repeatable media
verification. Their random passwords and the test's existing admin password
are stored only in mode-600 `~/.local/share/den-m6/smoke-credentials.json`.
The test sent a general-channel message, opened a DM, and posted a labeled test
clip in clips. These test artifacts remain identifiable in the public instance.

## Backups and restore

The VPS timer runs nightly at 06:00 UTC with up to five minutes of jitter and
`Persistent=true`. `backup.sh` takes a SQLite `VACUUM INTO` snapshot, validates
integrity, then rsyncs the snapshot and uploads over Tailscale to
`/mnt/storage/den/backups/YYYY-MM-DD/` on codexbox. A `complete` file containing
an ISO UTC timestamp is transferred last. During a retry that file says
`incomplete`; never restore a directory with that value.

The dedicated VPS key is `/var/lib/den/.ssh/backup_ed25519`, owned by `den`,
mode 600. Its pinned codexbox SSH host key was compared with codexbox's local
host-key fingerprint. The destination account is `den-backup`, home
`/var/lib/den-backup`, with no password. Its authorized key uses:

```text
from="100.81.174.46",restrict,command="/usr/bin/rrsync -wo -no-del /mnt/storage/den/backups"
```

Append the VPS key's public half to that forced-command prefix when recreating
the account. Do not copy Nicholas's private keys. The destination and its SSH
directory are mode 700, and authorized_keys is mode 600. The forced command
rejects arbitrary shell commands, disables SSH forwarding, confines writes to
the backup directory, and disallows remote deletion. This was checked with a
rejected remote `id` command as well as a successful backup transfer.

Codexbox's local prune timer runs at 08:00 UTC, retaining the current UTC date
and thirteen previous dates. It runs as `den-backup`. Deletion authority stays
on codexbox, outside the VPS's SSH credential. A backup of the uploaded clip was
transferred successfully and the prune service ran successfully.

For a fresh backup and a safe restore drill:

```bash
ssh vps 'sudo systemctl start den-backup'
./deploy/restore-check.py /mnt/storage/den/backups/2026-09-13 \
  ~/.local/share/den-m6/restore-token.json
```

Choose the actual backup date. The token JSON contains the output of
`den token create`; its hash must already be in the backed-up database. The
mode-600 token file above was created before the proven snapshot. Keep it
private. For a future drill, mint a token into a private file before taking the
backup, then revoke it with `den token revoke` when it is no longer needed.

The drill copies the snapshot into a private temporary directory on codexbox,
checks its completion marker, integrity, foreign keys, and all completed upload
sizes, boots the real release server on 127.0.0.1:17200, authenticates `/channels`
with the token, and verifies upload byte ranges. It stops the throwaway server
and removes the scratch copy afterward. The actual drill passed with six
visible channels, including the test DM, and the 41-second clip.

For an actual VPS restore:

1. Run the safe drill against the chosen completed snapshot first. Copy that
   snapshot into a private directory owned by Nicholas on codexbox using sudo
   rsync with `--chown`, then transfer it with ordinary `rsync` and `ssh vps` to
   `/home/debian/den-restore/`.
2. On the VPS, stop `den-backup.timer`, `den-backup.service`, and `den-server`.
   Preserve the current database, its WAL/SHM files, and uploads in a dated
   recovery directory outside `/var/lib/den`. Keep the current `.ssh` directory
   and `/etc/den/den.env` in place. Never mix the old WAL with a restored database.
3. Install the restored `den.db` as den:den mode 600 and the restored uploads
   directory as den:den under `/var/lib/den`. Start `den-server` and re-enable
   `den-backup.timer`. Check public health, log in, open channels, and play a clip
   before removing any recovery copy.

Database and upload backups do not include production configuration or SSH
private keys. A replacement VPS uses `install-vps.sh` to generate new LiveKit
credentials and obtains new Caddy certificates; recreate the restricted backup
key authorization separately.

## Monitoring and keys

Codexbox runs `deploy/monitor.py` every five minutes against the public health
URL. Two consecutive failed checks trigger one Telegram message; further
failures stay quiet; the first healthy check sends recovery. It calls the
existing `~/.local/bin/hermes send` command to Nicholas's existing Telegram
recipient. No bot token was duplicated. State lives in
`~/.local/state/den-monitor/state.json`.

The state transitions were checked, then a separate test state used an
unreachable local URL and the real public health URL. Hermes confirmed both
clearly labeled test messages. The public service was not stopped for that
drill. A delivery failure leaves the old state in place so the next check retries.

Fleet status now includes the Den health URL and LiveKit unit, listeners, and
signaling URL. [Fleet PR 8](https://github.com/nhclink16/fleet-skill/pull/8) merged
and synced to codexbox and the VPS. Both probes report healthy with HTTP 200.
The validator and 95 applicable local tests passed. GitHub-hosted jobs did not
start because of an account billing/spending-limit issue; hosted CI is not
claimed as passing.

To rotate LiveKit API credentials on the VPS:

```bash
sudo /opt/den/bin/rotate-livekit-keys.py
```

This generates a new key and secret, replaces them in both private config files
while preserving ownership and mode 600, and restarts LiveKit and Den. Active
calls disconnect and must rejoin. Then check public health and repeat the TURN
smoke. It never prints the credentials. This helper is installed but was not run
during deployment, which already generated fresh production keys.

## Verification evidence

Passed against the public URL on 2026-09-13:

- Release Rust binaries and web build. Installed binary hashes match codexbox.
- Both DNS names through 1.1.1.1; valid public certificates; app `curl -I` returns
  HTTP/2 200; `/health` is healthy; `/openapi.json` loads with 34 paths.
- CLI upload of a newly generated 41-second 1280×720 H.264/AAC MP4, 16,487,280
  bytes. The historical test clip was absent. A 1 MiB range request returned
  206 and exactly matched the source bytes. Browser playback decoded video,
  advanced past one second, and reported duration 41 seconds.
- M3 fake-media Chromium smoke: three cameras, screen sharing, received audio
  and playing audio elements, remote mute, PTT and custom key, blur, chat during
  a call, DM privacy and switching, reload/resync, leave cleanup, and desktop
  and mobile screenshots. The expected media peer was `135.148.120.197`.
- Relay-only ICE via LiveKit's `rtcConfig: { iceTransportPolicy: 'relay' }`.
  Both transports selected `relay`; both participants received audio packets
  and decoded video. A second run retained only TURN/TLS candidates and selected
  `turns:rtc.den.nicholascaron.com:443?transport=tcp` with relay protocol `tls`.
- Public traffic used codexbox's normal `enp5s0` route through its home gateway;
  selected media peers were public VPS addresses. These tests did not use
  Tailscale HTTP or media routes. They used separate browser contexts on one
  physical codexbox, not two people on separate physical networks.
- Completed off-box backup and real-server restore drill, including the clip.
- Dedicated service users, hardening settings, private file modes, enabled
  timers, post-firewall SSH, certificate-copy checks, and live fleet probes.
- Desktop call, mobile call, and clip screenshots were opened and inspected:
  [desktop call](shots/m6-call-desktop.png), [mobile call](shots/m6-call-mobile.png),
  [clip](shots/m6-clip-desktop.png). The green test pattern is Chromium fake media.

Re-run call verification when hangout is empty, because the M3 script checks
exact participant counts and signs in as Nicholas:

```bash
DEN_SMOKE_URL=https://den.nicholascaron.com \
DEN_SMOKE_CREDENTIALS="$HOME/.local/share/den-m6/smoke-credentials.json" \
DEN_SMOKE_USERS='["nicholas","m6_bob","m6_ari"]' \
DEN_SMOKE_MEDIA_IP=135.148.120.197 \
DEN_SMOKE_SHOTS="file://$HOME/.local/share/den-m6/shots/" \
node scripts/m3-smoke.mjs

DEN_SMOKE_CREDENTIALS="$HOME/.local/share/den-m6/smoke-credentials.json" \
node scripts/m6-turn-smoke.mjs

DEN_TURN_TLS=1 \
DEN_SMOKE_CREDENTIALS="$HOME/.local/share/den-m6/smoke-credentials.json" \
node scripts/m6-turn-smoke.mjs
```

Local logs are `/tmp/den-m6-smoke.log`, `/tmp/den-m6-turn.log`, and
`/tmp/den-m6-turn-tls.log`. Screenshots and private verification inputs live in
`~/.local/share/den-m6/`. Nicholas's detailed phone feedback and the human
cross-network audio/video check are the remaining acceptance evidence.
