# Minecraft activity relay

Shows "Playing Minecraft" on Den for everyone logged into the Minecraft server.
It runs next to the Minecraft server (it needs RCON), polls `list` every 30
seconds, and sets a `minecraft` activity for each player it can match to a Den
account. Activities expire after 90 seconds, so a stopped server or relay never
leaves anyone "playing".

## Setup

1. Create an API token for an admin account, since the relay sets other people's
   activities: `den token create minecraft-relay`.
2. Put it in `~/.config/den/minecraft-activity.env`:

   ```
   DEN_URL=https://denchat.app
   DEN_TOKEN=<token>
   ```

3. If a Minecraft name differs from the Den username, map it in
   `~/.config/den/minecraft-players.json`: `{"Steve_1984": "maya"}`.
4. Check it once, then install the unit:

   ```
   set -a; . ~/.config/minecraft/rcon.env; . ~/.config/den/minecraft-activity.env; set +a
   ./den-minecraft-activity --once
   cp den-minecraft-activity.service ~/.config/systemd/user/
   systemctl --user enable --now den-minecraft-activity
   ```
