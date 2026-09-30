// Game servers a relay reports on: Minecraft first, another game later. The
// relay declares its buttons and numbers; clients render only what it declared,
// so a second game needs a new relay and no Den changes.

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::Id;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ServerState {
    Up,
    Starting,
    Stopping,
    /// Stopped to save resources while nobody plays; wakes on join or Start.
    Asleep,
    Down,
}

/// A button the relay can carry out.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, ToSchema)]
pub struct ServerAction {
    /// What the relay receives back in a command: `save`, `start`, `restart`, `stop`.
    #[schema(max_length = 32)]
    pub id: String,
    #[schema(max_length = 32)]
    pub label: String,
    /// Only admins may run it. Den enforces this, not the client.
    pub admin_only: bool,
    /// The states in which the button is offered.
    pub states: Vec<ServerState>,
}

/// How a client formats a number.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum StatUnit {
    Number,
    /// Ticks per second.
    Tps,
    Bytes,
    Percent,
    /// A duration, such as uptime.
    Seconds,
    /// Unix seconds, such as the last backup.
    Timestamp,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, ToSchema)]
pub struct ServerStat {
    /// Stable name: `tps`, `memory`, `cpu`, `uptime`, `backup`.
    #[schema(max_length = 32)]
    pub key: String,
    #[schema(max_length = 32)]
    pub label: String,
    pub unit: StatUnit,
    pub value: f64,
    /// Den keeps 24 hours of this stat for a graph. Player counts always are.
    #[serde(default)]
    pub graph: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, ToSchema)]
pub struct ServerPlayer {
    /// The in-game name.
    pub name: String,
    /// The Den account the relay matched, if any.
    pub user_id: Option<Id>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, ToSchema)]
pub struct ServerPlaytime {
    pub name: String,
    pub user_id: Option<Id>,
    /// All-time, from the game's own records. The relay sends it.
    pub total_seconds: Option<i64>,
    /// The past 7 days, from Den's record of who was online. Den fills it in;
    /// a relay leaves it out.
    #[serde(default)]
    pub week_seconds: i64,
}

/// What a relay says about its server when it connects, and again whenever it changes.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, ToSchema)]
pub struct ServerInfo {
    /// Stable id, part of the page URL: lowercase letters, digits and hyphens.
    #[schema(max_length = 32)]
    pub slug: String,
    /// Which game, for the fallback icon: `minecraft`.
    #[schema(max_length = 32)]
    pub game: String,
    /// Display name, such as the Minecraft MOTD without formatting codes.
    #[schema(max_length = 100)]
    pub name: String,
    /// Short facts shown under the name: `1.21.1`, `Fabric`, `56 mods`.
    pub details: Vec<String>,
    /// What players type to join.
    pub address: Option<String>,
    pub actions: Vec<ServerAction>,
    /// Base64 PNG, at most 64 KiB decoded.
    #[serde(default)]
    pub icon_png: Option<String>,
}

/// The live half, sent every few seconds.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, ToSchema)]
pub struct ServerStatus {
    pub state: ServerState,
    pub players: Vec<ServerPlayer>,
    pub max_players: Option<u32>,
    pub stats: Vec<ServerStat>,
    /// All-time totals. Absent means unchanged since the last status.
    #[serde(default)]
    pub playtime: Option<Vec<ServerPlaytime>>,
}

/// JSON text frames on `GET /servers/relay`. The relay sends `hello` first, then
/// `status` whenever it polls; Den sends `command` when someone presses a button
/// and the relay answers each with `result`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, ToSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RelayFrame {
    Hello {
        server: ServerInfo,
    },
    Status {
        status: ServerStatus,
    },
    Command {
        command_id: Id,
        action: String,
        /// Display name of whoever pressed it, for an in-game line.
        by: String,
    },
    Result {
        command_id: Id,
        ok: bool,
        message: Option<String>,
    },
}

/// A game server as clients see it.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, ToSchema)]
pub struct GameServer {
    pub slug: String,
    pub game: String,
    pub name: String,
    pub details: Vec<String>,
    pub address: Option<String>,
    pub icon_url: Option<String>,
    /// False when the relay is not connected. The state is then `down`, and no
    /// action can run until it reconnects.
    pub connected: bool,
    pub state: ServerState,
    pub players: Vec<ServerPlayer>,
    pub max_players: Option<u32>,
    pub stats: Vec<ServerStat>,
    pub actions: Vec<ServerAction>,
    /// Unix seconds of the last status.
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, ToSchema)]
pub struct ServerPoint {
    /// Unix seconds.
    pub at: i64,
    pub value: f64,
}

/// 24 hours of one number, a point per minute at most.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, ToSchema)]
pub struct ServerSeries {
    /// `players`, or a stat key the relay marked `graph`.
    pub key: String,
    pub label: String,
    pub unit: StatUnit,
    pub points: Vec<ServerPoint>,
}

/// `GET /servers/{slug}`: the server plus what only its page needs.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, ToSchema)]
pub struct ServerDetail {
    pub server: GameServer,
    /// Everyone who has played, most all-time first.
    pub playtime: Vec<ServerPlaytime>,
    pub history: Vec<ServerSeries>,
}

/// What `POST /servers/{slug}/actions/{action}` returns once the relay has done it.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, ToSchema)]
pub struct ServerActionResult {
    pub message: Option<String>,
}

/// A server slug: lowercase letters, digits and hyphens, starting with a letter or digit.
pub fn valid_server_slug(slug: &str) -> bool {
    crate::valid_activity_slot(slug)
}
