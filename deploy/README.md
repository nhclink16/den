# Public VPS

The live instance is https://denchat.app. Run `./deploy/release.sh`
from codexbox to build and redeploy. The VPS runs Den, LiveKit 1.9.0, Caddy,
and HAProxy as systemd services. See [M6 notes](../docs/M6-NOTES.md) for setup,
backups, restore, certificate renewal, verification, and key rotation.

`compose.yml` remains a historical Docker outline. It uses `Caddyfile.compose`;
the production systemd deployment uses `Caddyfile`.

# LiveKit on the dev box

The demo is https://codexbox.tail44c455.ts.net:4200. LiveKit signaling is
wss://codexbox.tail44c455.ts.net:4202. Media travels directly over Tailscale to
100.116.27.23, UDP 50000–50200, with TCP 7881 as fallback. Signaling listens on
loopback 7880. ICE candidates are restricted to the tailnet IP. Both clients must be on the tailnet. TURN is disabled for this
setup; a public VPS needs its own signaling domain and TURN certificates.

Create a private environment file at `~/.config/den/livekit.env` containing
`DEN_LIVEKIT_API_KEY`, `DEN_LIVEKIT_API_SECRET` with a random secret of at least
32 bytes, `DEN_LIVEKIT_URL=wss://codexbox.tail44c455.ts.net:4202`, and
`LIVEKIT_NODE_IP=100.116.27.23`. Never commit that file. Den reads the three
`DEN_LIVEKIT_*` variables. Missing configuration leaves text working and token
requests return 503.

Run from the repository root, with Docker and `envsubst` installed:

```bash
set -a
source "$HOME/.config/den/livekit.env"
set +a
umask 077
envsubst '${LIVEKIT_NODE_IP} ${DEN_LIVEKIT_API_KEY} ${DEN_LIVEKIT_API_SECRET}' \
  < deploy/livekit.yaml > "$HOME/.config/den/livekit.yaml"
docker run -d --name den-livekit --restart unless-stopped --network host \
  -v "$HOME/.config/den/livekit.yaml:/etc/livekit.yaml:ro" \
  livekit/livekit-server:v1.9.0 --config /etc/livekit.yaml
tailscale serve --bg --https=4202 http://127.0.0.1:7880
```

The YAML is a template; LiveKit does not expand environment variables in YAML.
The rendered file is mode 600. Its webhooks go to the loopback dev server on
7000 and the demo on 7200. Each Den database seeds its own hangout ID so the
instances do not share rooms. An offline dev server may produce webhook retry
warnings while the demo continues working.

The demo reads the environment file via
`~/.config/systemd/user/den-demo.service.d/voice.conf`:

```ini
[Service]
EnvironmentFile=%h/.config/den/livekit.env
```

After building the client and server, restart the demo:

```bash
npm --prefix apps/web ci
npm --prefix apps/web run build
cargo build --release -p den-server
systemctl --user daemon-reload
systemctl --user restart den-demo
```

For local Vite development, use `DEN_ORIGIN=http://localhost:5173` and
`DEN_LIVEKIT_URL=ws://localhost:7880`. Run the dev API on 7000 and `npm run dev`
in `apps/web`. Vite proxies `/calls` and `/livekit` as well as the existing API.
A browser on another tailnet device must use the HTTPS demo URL for microphone
and camera access.

## Spotify Jam account links

Jam cards work without Spotify credentials. To show a host's current track, add
these private values to the server environment:

```ini
DEN_SPOTIFY_CLIENT_SECRET=<Spotify developer app secret>
DEN_SPOTIFY_KEY_FILE=/var/lib/den/spotify.key
```

The client ID is public and compiled into the server. The server creates the
data key as a mode-0600 regular file and refuses an existing key readable by
group or other users. Never commit either value. Production systemd reads the
client secret from `/etc/den/den.env`; `ProtectHome=true` prevents it from
reading a user's `~/.config` copy.

Spotify has registered `https://denchat.app/spotify/callback` and
`http://127.0.0.1:5173/spotify/callback`. Local OAuth therefore requires
`DEN_ORIGIN=http://127.0.0.1:5173` and the browser must use that exact host.
Spotify rejects `localhost` for this app.

Offline exports include encrypted refresh-token rows. A normal import deletes
them. `--keep-credentials` retains them for disaster recovery, which also
requires restoring the same `DEN_SPOTIFY_KEY_FILE` separately with mode 0600.

Migrations run automatically through `den-server`. Migration 0003 rebuilds the
channel table to allow voice rooms. The startup migrator disables foreign keys
on its migration connection, validates references, then re-enables them before
serving. Do not apply this table rebuild using a different migrator with foreign
keys enabled; SQLite would cascade the parent-table deletion into chat data.

Configuration fields follow the [LiveKit v1.9.0 sample](https://github.com/livekit/livekit/blob/v1.9.0/config-sample.yaml).
