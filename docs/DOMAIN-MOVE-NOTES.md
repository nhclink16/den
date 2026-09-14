# Den domain move

Completed 2026-09-14 UTC. The app is https://denchat.app and LiveKit signaling
is wss://rtc.denchat.app. Public health reports 0.1.2. All three enrolled hosts
are connected, and the required browser, terminal and TURN checks passed.

Everyone must log in once more at the new domain. The old host's cookies do not
transfer. Nicholas confirmed that login works. Passwords, API/bot credentials,
host credentials and LiveKit keys are unchanged. Existing bearer authentication
was verified against the new origin. API clients should use the new base URL;
a cross-host redirect is not a way to carry Authorization headers.

## Deployment

`den-backup.service` succeeded before changes, at 00:11:23 UTC, using the existing
off-box archive destination and retention. Private configuration rollback copies
are under `/root/den-domain-move` on the VPS. Both new DNS-only A records resolved
through 1.1.1.1 to 135.148.120.197. SSH identity was verified as vps-2fd9743a.

`/etc/den/den.env` now has `DEN_ORIGIN=https://denchat.app` and
`DEN_LIVEKIT_URL=wss://rtc.denchat.app`. An exact comparison with the private
backup confirmed both LiveKit credential lines unchanged. The LiveKit YAML
changed only its TURN domain. Caddy serves the new app and signaling names;
HAProxy routes non-HTTP TLS for rtc.denchat.app to LiveKit on 5349, allowing
clients to use TURN/TLS on public 443.

The certificate sync helper now selects rtc.denchat.app. Its certificate is
valid through 2026-12-12; the timer is active and an unchanged-certificate rerun
passed. Caddy, HAProxy, LiveKit and Den are active. The deployed Caddy, HAProxy
and certificate-helper files match the committed deployment files by SHA-256.
No application rebuild or new release was needed.

The initial cutover guards caught different LiveKit environment-key names and
Windows' extended-path prefix before those mutations ran. Correcting the checks
completed the origin change and host restart. There was a brief login interruption
during cutover; Nicholas then confirmed successful login.

## Redirects and shared links

`curl -I` returned:

| Host | Result |
| --- | --- |
| denchat.app | HTTPS 200 |
| rtc.denchat.app | HTTPS 200 |
| den.nicholascaron.com | HTTPS 301 to https://denchat.app/ |
| rtc.den.nicholascaron.com | HTTPS 301 to https://rtc.denchat.app/ |

Both redirects preserve path and query. A browser also verified that an old
`/settings/machines?domain_move=check#preserved` link reached the same path, query
and fragment on the new origin. An invite-shaped login query was preserved.
Keep the old DNS names and Caddy certificate/redirect blocks in place for shared
links. Old TURN connections must reconnect using the new RTC endpoint.

Settings, Machines already derives installer URLs from `location.origin`, so no
hardcoded client edit was needed. The actual panel showed all three hosts online;
its generated shell and PowerShell commands use the new domain, and the enrollment
code embeds the new origin. The verification code was hidden afterward and will
expire normally; no host was enrolled again.

README, DESIGN, deployment instructions and smoke defaults use the new names.
Historical M6 and M8 notes remain unchanged. Old names remain where they identify
redirect sources, including this report and the move brief's requirements.

## Host and media evidence

The existing `host.toml` files were updated in place on codexbox, the iMac and
Windows PC, preserving every field except the origin. Private copies named
`host.toml.before-domain-move` remain beside them. Their systemd user service,
launchd agent and Den Host Scheduled Task were restarted respectively. The server
reports exactly the same three host IDs as before the move.

The M8 terminal smoke ran against https://denchat.app and checked actual keyboard
input through the remote PTY and Ghostty output, then closed each test session:

| Host | Passing session |
| --- | --- |
| codexbox | 01M2EMC0JAP9D68MM6MHF1WH4B |
| Nicholas-Work.local | 01M2EME2T4VD3CZZQX3E1S0JRJ |
| Windows, nicholas | 01M2EMFC3HD271N39WJM5JYSN6 |

The first iMac attempt produced the correct remote output but reported one browser
`offset is out of bounds` error. A fresh rerun passed with no browser errors.
The smoke now records error stacks. The pre-existing missing zoxide/atuin shell
helpers remain as documented in M8; terminal commands execute normally.

After Nicholas left hangout, `scripts/m3-smoke.mjs` passed membership/mute/leave,
three cameras, screen sharing, received audio, PTT/custom key/blur, chat, DM privacy
and switching, reload/resync, and desktop/mobile checks. Its expected media peer
was the public VPS address. Both `scripts/m6-turn-smoke.mjs` runs passed received
audio and decoded video. Relay-only selected UDP on 3478; TLS-only selected
`turns:rtc.denchat.app:443?transport=tcp` with relay protocol `tls` for both
transports. These used Chromium fake media over the public network, not a human
two-network call. Test participants disconnected afterward.

Screenshots were opened and inspected: [Machines](shots/domain-move-machines.png),
[desktop call](shots/domain-move-call-desktop.png),
[mobile call](shots/domain-move-call-mobile.png). Full smoke logs and terminal
screenshots remain in private `/mnt/storage/den-domain-move` on codexbox.

## Monitoring and fleet

Codexbox's existing monitor now checks https://denchat.app/health. Its real service
run succeeded with zero failures and no active alert. The systemd unit already
points to the repository script, so no scheduler replacement was needed.

[Fleet PR 9](https://github.com/nhclink16/fleet-skill/pull/9) merged as d4d9b168.
The validator and all 136 local tests passed. GitHub-hosted fleet jobs did not
start because the private repository's account payment/spending limit blocked
them; hosted fleet CI is not claimed as passing. Publishing used the human HTTPS
credential after the read-only autosync key correctly refused a push.

Native fleet sync completed on codexbox and the VPS. Each host's Codex, Claude
and OpenCode mirror passed verification of all 60 manifest files. Live Den and
LiveKit fleet probes report healthy, active services and HTTP 200 at the new
URLs. Shell/Python/JavaScript syntax checks passed for the changed scripts.

Den changes were committed to main and pushed with explicit paths. Unrelated
design work was untouched. The domain-move work stops here.
