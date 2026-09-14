# Move Den to denchat.app

Owner: Astra. Read `docs/M6-NOTES.md` and `docs/M8-NOTES.md` first. DNS is done: `denchat.app` and `rtc.denchat.app` are DNS-only A records at the VPS and resolve.

Done when: `https://denchat.app` is the app, voice works there including TURN over 443 on `rtc.denchat.app`, the three enrolled hosts (codexbox, iMac, Windows PC) are connected to the new origin, `https://den.nicholascaron.com` redirects to `https://denchat.app` with a 301 preserving the path, and nothing Nicholas has shared breaks except that everyone logs in once more.

- Back up first (`den-backup`), then change `DEN_ORIGIN` to `https://denchat.app` and LiveKit's URL to `wss://rtc.denchat.app` in `/etc/den/den.env`. Do not rotate LiveKit keys.
- Caddy: serve the app on `denchat.app`, LiveKit signaling on `rtc.denchat.app`, and add 301 redirects from `den.nicholascaron.com` and `rtc.den.nicholascaron.com` to the new names. HAProxy SNI routing and the TURN certificate sync move to the new RTC name; keep `deploy/` in sync with what runs.
- Sessions are cookies on the old host, so every user logs in again at the new domain. Say so plainly in the notes. Tokens, hosts, and bot credentials are unaffected.
- Enrolled hosts store the server URL in `host.toml`. Update it to the new origin on codexbox (local), the iMac (`ssh imac`), and the Windows PC (`ssh pc`), restart each host service, and confirm all three show connected in Settings, Machines on the new domain. No re-enrollment.
- Update the codexbox health monitor URL, the fleet skill probes (PR to the fleet-skill repo as before), the installer one-liner shown in Settings, `README.md`, and every `den.nicholascaron.com` in `docs/` and `deploy/` except the historical M6 and M8 notes. Update `docs/DESIGN.md` hosting line.
- Verify: `curl -I` on all four hostnames, the M3 smoke and the M6 TURN smoke against `https://denchat.app`, a terminal opened on each of the three hosts, and public health. Write `docs/DOMAIN-MOVE-NOTES.md` with evidence and stop.
