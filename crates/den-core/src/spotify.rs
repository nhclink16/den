use crate::{nullable_object_schema, Id};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// Whether a user's Spotify account is attached to Den, and whether it still works.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum SpotifyConnection {
    /// This server holds no Spotify client secret, so there is nothing to offer.
    Unavailable,
    Disconnected,
    Connected,
    /// Spotify rejected the stored refresh token. Refresh tokens die after 180
    /// days, so this is a normal state to re-authorise from, not an error.
    Reauthorize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
pub struct SpotifyAccount {
    pub connection: SpotifyConnection,
    /// Spotify display name, for "Connected as …". Absent unless connected.
    pub account_name: Option<String>,
    pub connected_at: Option<i64>,
    /// Unix seconds when the refresh token is assumed dead, so the UI can warn
    /// before the cliff. Advisory: a rejection from Spotify is authoritative.
    pub expires_at: Option<i64>,
    /// Whether everyone sees "Listening to …" while this person is online. Off
    /// until they turn it on; connecting for Jams alone never shares it.
    #[serde(default)]
    pub share_listening: bool,
}

/// Body for `PUT /users/me/spotify/sharing`.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct SpotifySharing {
    pub share_listening: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct SpotifyAuthorization {
    /// Send the browser here. The code comes back to `/spotify/callback`.
    pub url: String,
    pub expires_in: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CompleteSpotifyAuth {
    pub code: String,
    pub state: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct StartJam {
    pub url: String,
}

/// A Spotify Jam someone started for a room. Pinned, not a message.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
pub struct Jam {
    pub id: Id,
    pub channel_id: Id,
    pub url: String,
    pub host_id: Id,
    pub started_at: i64,
    /// Den members who pressed Join. Spotify never reports who is listening, so
    /// this is a count of Den clicks and nothing more.
    pub joined_user_ids: Vec<Id>,
    /// Only the host's own playback is readable, and only once the host has
    /// connected Spotify. Absent otherwise.
    #[schema(schema_with = nullable_object_schema::<SpotifyNowPlaying>)]
    pub now_playing: Option<SpotifyNowPlaying>,
}

/// A room's Jam, or none. Wrapped rather than returned as a bare nullable
/// object, because Apple's Swift generator rejects a standalone null branch in
/// `oneOf`. Mirrors the shape of `Event::JamUpdated`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
pub struct RoomJam {
    #[schema(schema_with = nullable_object_schema::<Jam>)]
    pub jam: Option<Jam>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
pub struct SpotifyNowPlaying {
    pub track: String,
    pub artists: String,
    pub album_art: Option<String>,
    pub duration_ms: Option<i64>,
    pub progress_ms: Option<i64>,
    pub is_playing: bool,
    /// Server clock for `progress_ms`, Unix milliseconds. Mirrors `MusicQueue::updated_at`.
    pub sampled_at: i64,
}
