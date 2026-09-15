use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum MusicTrackState {
    Queued,
    Loading,
    Playing,
    Paused,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, ToSchema)]
pub struct MusicTrack {
    pub id: String,
    pub room_id: String,
    pub url: String,
    pub title: String,
    pub duration: Option<f64>,
    pub thumbnail: Option<String>,
    pub added_by: String,
    pub position: i64,
    pub state: MusicTrackState,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, ToSchema)]
pub struct MusicQueue {
    pub room_id: String,
    /// Stable LiveKit participant identity; use the regular participant volume control.
    pub participant_id: String,
    pub queue: Vec<MusicTrack>,
    pub paused: bool,
    pub position_seconds: f64,
    /// Server time for the progress position, in Unix milliseconds.
    pub updated_at: i64,
    pub revision: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct AddMusic {
    pub url: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct PauseMusic {
    pub paused: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct SeekMusic {
    pub position_seconds: f64,
}
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct OrderMusic {
    pub ids: Vec<String>,
}
