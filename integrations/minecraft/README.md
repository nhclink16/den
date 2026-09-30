# Minecraft relay

Feeds Den's Servers page over one outbound WebSocket, reports status every 10
seconds, and handles Save world, Start, Restart, and Stop buttons. Den restricts
Restart and Stop to admins. The relay also refreshes "Playing Minecraft"
activities every 30 seconds for matched players, with a 90-second TTL, and clears
activities when players leave.

## Setup

Run the relay next to Minecraft, with access to its files and RCON through
`mcrcon`. Install Python and `mcrcon`, then create the relay's virtual environment:

```sh
python3 -m venv ~/.local/share/den-minecraft-relay/venv && ~/.local/share/den-minecraft-relay/venv/bin/pip install websockets
```

In Den, sign in to an **ADMIN** account, go to Settings › Agents / API keys, and
create a token named `minecraft-relay`. Put it in
`~/.config/den/minecraft-activity.env`:

```sh
DEN_URL=https://denchat.app
DEN_TOKEN=<token>
```

Keep the existing RCON settings in `~/.config/minecraft/rcon.env`:

```sh
RCON_HOST=127.0.0.1
RCON_PORT=25575
RCON_PASSWORD=<password>
```

The relay passes RCON credentials through `MCRCON_HOST`, `MCRCON_PORT`, and
`MCRCON_PASS`, keeping the password out of command arguments.

If a Minecraft name differs from the Den username, map it in
`~/.config/den/minecraft-players.json`, for example `{"Steve_1984": "maya"}`.
Unmapped names match Den usernames without regard to case. The Den user list is
cached for five minutes; a failed lookup leaves status reporting available.

These variables can also go in `minecraft-activity.env`:

| Variable | Default / purpose |
| --- | --- |
| `MCRCON` | `mcrcon` executable on PATH |
| `DEN_MINECRAFT_PLAYERS` | `~/.config/den/minecraft-players.json` mapping file |
| `MC_SERVER_DIR` | `~/minecraft/server` |
| `MC_UNIT` | `minecraft` systemd unit |
| `SYSTEMCTL` | `systemctl` executable on PATH, used for status and PID lookup |
| `MC_BACKUP_DIR` | `/mnt/storage/minecraft/backups`; newest `world-*` file's timestamp |
| `MC_ADDRESS` | Optional join address shown with a Copy button |
| `DEN_SERVER_SLUG` | `minecraft` |
| `MC_START` | Shell command, default `sudo -n systemctl --no-block start <MC_UNIT>` |
| `MC_STOP` | Shell command, default `sudo -n systemctl --no-block stop <MC_UNIT>` |
| `MC_RESTART` | Shell command, default `sudo -n systemctl --no-block restart <MC_UNIT>` |

The default control commands need existing passwordless sudo permission. Each
has a 60-second timeout. Save uses RCON `save-all flush`. Restart first announces
who pressed the button in Minecraft, then runs `MC_RESTART`.

From `integrations/minecraft`, check the setup with `--once`. This polls the
configured server and prints hello and status as two JSON lines, without opening
a WebSocket or changing activities:

```sh
set -a
. ~/.config/minecraft/rcon.env
. ~/.config/den/minecraft-activity.env
set +a
~/.local/share/den-minecraft-relay/venv/bin/python ./den-minecraft-relay --once
```

Install the renamed unit after stopping/disabling any old activity relay:

```sh
mkdir -p ~/.config/systemd/user
cp den-minecraft-relay.service ~/.config/systemd/user/
systemctl --user daemon-reload
systemctl --user enable --now den-minecraft-relay
```

The unit expects the checkout at `~/den`. It keeps the existing environment file
paths. On each connection the relay sends hello first to `/servers/relay`, then
status, and reconnects with a delay from 1 to 60 seconds. It resends hello when
metadata changes. All-time playtime comes from world stats on connect and every
five minutes. Available stats include Spark TPS, process memory, CPU as a
percentage of the whole machine, process uptime, and the last backup timestamp.
Unavailable stats are omitted.

Each server belongs to its registering relay key until that key is revoked;
ownership refusals wait 60 seconds, and reconnect backoff resets only after a
session lasts at least 60 seconds.
The relay remembers the last 100 command IDs across reconnects, while Den keeps
one action pending per server through HTTP timeouts until a result, disconnect,
or five-minute backstop.

## Tests

Tests create temporary Minecraft files and fake executables, and run a fake Den
on localhost. They never use the live server or its RCON credentials.

```sh
python3 -m venv /tmp/relay-venv && /tmp/relay-venv/bin/pip install -q websockets
/tmp/relay-venv/bin/python -m unittest integrations/minecraft/test_relay.py -v
python3 -m py_compile integrations/minecraft/den-minecraft-relay
```
