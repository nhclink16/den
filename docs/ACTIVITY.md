# Activity

What people are doing right now: "Playing Minecraft", "Listening to Midnight City",
"Using Blender", "Working on Backups". Shown under names in the people list and on
profile cards.

## Model

Types live in `crates/den-core/src/activity.rs`. An `Activity` has a `slot`, a
`kind` (`playing`, `listening`, `watching`, `using`, `working`), a `name` (1–64
characters, one line), optional `details` (≤128), an optional `image_url` (only the
server sets it), `started_at` and `expires_at` in Unix milliseconds.

A person can hold several at once, one per slot. The slot names the source:
`desktop`, `spotify`, `minecraft`, `cli`, or any `[a-z0-9-]{1,32}` a script picks.

Activities are held in memory like presence, not in SQLite. Every activity expires
unless its source refreshes it (default TTL 90 seconds, 10 s–24 h), so a crashed
app or a stopped relay never leaves someone "playing". A refresh of the same kind
and name keeps `started_at`. The server sweeps expired entries every 5 seconds.

## API

- `PUT /users/{id}/activities/{slot}` with `SetActivity {kind, name, details?, ttl_seconds?}`.
  `id` is `me` or a user id. Setting someone else's needs an admin: the Minecraft
  relay runs with an admin token. `spotify` is reserved for the server.
- `DELETE /users/{id}/activities/{slot}`, same rules.
- `GET /activities` lists everyone's current activities; `GET /presence` carries
  the same list as `activities` (absent from older servers, so clients default it).
- `activity_updated {user_id, activities}` goes to every socket, like `presence`,
  and replaces that person's whole list. An empty list means nothing.

## Sources

- **Desktop.** The Electron app reads the focused window every 15 seconds
  (`apps/desktop/electron/activity.cjs`): `xprop` on X11/XWayland, `lsappinfo` on
  macOS (no permission prompt), PowerShell on Windows. Only the app's name is
  sent. Window titles and command lines are read locally, only to recognise a
  game: Steam's `steamapps/common/<Game>` and Minecraft become `playing`, anything
  else `using`. Desktop shells and lock screens are ignored. When Den itself is
  focused the previous answer stands, so alt-tabbing to chat mid-game keeps
  "Playing". Ten idle minutes, a locked screen or sleep clear it. Sharing is on
  by default; Settings > Profile > Activity turns it off or hides individual apps,
  per device. Native Wayland windows are not visible to `xprop`.
- **Spotify.** Every 20 seconds the server polls "currently playing" for people who
  are online and have Spotify connected, and fills the `spotify` slot with the
  track, artists and album art.
- **Minecraft.** `integrations/minecraft/den-minecraft-activity` runs beside the
  Minecraft server, reads `list` through `mcrcon`, and sets a `minecraft` activity
  for each player it can match to a Den account.
- **Agents and scripts.** `den activity set working "Backups" --details "…" --ttl 600`,
  `den activity clear`, `den activity list`. `--slot` and `--user` choose the slot
  and (for admins) the person.
