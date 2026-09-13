use crate::Id;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// UTF-8 JSON inside binary WebSocket messages. Byte fields contain octets.
/// Open is idempotent: an existing PTY sends its recent scrollback and resizes.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
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
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct HostLogin {
    pub code: String,
    pub name: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct HostCredential {
    pub server_url: String,
    pub host_id: Id,
    pub name: String,
    pub token: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct DirectCheck {
    pub token: String,
    pub session_id: Id,
    pub input: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct DirectPermission {
    pub user_id: Id,
    pub name: String,
    pub owner: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
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
