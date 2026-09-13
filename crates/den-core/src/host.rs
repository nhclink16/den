use crate::Id;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// UTF-8 JSON inside binary WebSocket messages. Byte fields contain octets.
/// Open is idempotent: an existing PTY sends its recent scrollback and resizes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum HostFrame {
    Open {
        session_id: Id,
        cols: u16,
        rows: u16,
        shell: Option<String>,
    },
    Input {
        session_id: Id,
        bytes: Vec<u8>,
    },
    Resize {
        session_id: Id,
        cols: u16,
        rows: u16,
    },
    Close {
        session_id: Id,
    },
    Output {
        session_id: Id,
        bytes: Vec<u8>,
    },
    Exited {
        session_id: Id,
        code: u32,
    },
    Ack {
        session_id: Id,
        bytes: usize,
    },
    Viewer {
        session_id: Id,
        user_id: Id,
        name: String,
    },
    Hello {
        direct_url: Option<String>,
    },
    Replay {
        session_id: Id,
        connection_id: Id,
    },
    Scrollback {
        session_id: Id,
        connection_id: Id,
        bytes: Vec<u8>,
    },
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
pub struct HostLogin {
    pub code: String,
    pub name: String,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
pub struct HostCredential {
    pub server_url: String,
    pub host_id: Id,
    pub name: String,
    pub token: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
pub struct DirectCheck {
    pub token: String,
    pub session_id: Id,
    pub input: bool,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
pub struct DirectPermission {
    pub user_id: Id,
    pub name: String,
    pub owner: bool,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TerminalFrame {
    TerminalInput {
        session_id: Id,
        bytes: Vec<u8>,
    },
    TerminalResize {
        session_id: Id,
        cols: u16,
        rows: u16,
    },
    TerminalOpen {
        session_id: Id,
    },
    TerminalClose {
        session_id: Id,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
pub struct Host {
    pub id: Id,
    pub owner_id: Id,
    pub name: String,
    pub online: bool,
    pub last_seen: Option<i64>,
    pub direct_url: Option<String>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
pub struct HostEnrollment {
    pub code: String,
    pub expires_at: i64,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum Capability {
    TerminalView,
    TerminalControl,
}
impl Capability {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::TerminalView => "terminal_view",
            Self::TerminalControl => "terminal_control",
        }
    }
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
pub struct Grant {
    pub id: Id,
    pub host_id: Id,
    pub grantee_id: Id,
    pub capability: Capability,
    pub expires_at: Option<i64>,
    pub created_by: Id,
    pub created_at: i64,
    pub revoked_at: Option<i64>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
pub struct RequestAccess {
    pub capability: Capability,
    pub duration_minutes: Option<u32>,
    #[serde(default)]
    pub standing: bool,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
pub struct AccessRequest {
    pub id: Id,
    pub host_id: Id,
    pub host_name: String,
    pub owner_id: Id,
    pub requester_id: Id,
    pub capability: Capability,
    pub duration_minutes: Option<u32>,
    pub standing: bool,
    pub status: String,
    pub expires_at: i64,
    pub grant_id: Option<Id>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
pub struct AccessDecision {
    pub allow: bool,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
pub struct AccessLog {
    pub id: Id,
    pub host_id: Id,
    pub actor_id: Id,
    pub action: String,
    pub subject_id: Option<Id>,
    pub created_at: i64,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
pub struct OpenTerminal {
    pub channel_id: Option<Id>,
    pub cols: Option<u16>,
    pub rows: Option<u16>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
pub struct TerminalState {
    pub id: Id,
    pub host_id: Id,
    pub host_name: String,
    pub owner_id: Id,
    pub cols: u16,
    pub rows: u16,
    pub active_controller_id: Option<Id>,
    pub viewer_ids: Vec<Id>,
    pub ended_at: Option<i64>,
    pub started_at: i64,
    pub recording_upload_id: Option<Id>,
    pub recording_capped: bool,
    pub control_request_ids: Vec<Id>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
pub struct SetController {
    pub user_id: Option<Id>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
pub struct ShareTerminal {
    pub channel_id: Id,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
pub struct TerminalWrite {
    pub text: String,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
pub struct DirectToken {
    pub token: String,
    pub url: Option<String>,
    pub expires_at: i64,
}
