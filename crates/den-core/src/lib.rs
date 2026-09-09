//! Shared API types. The server serializes these, the CLI and the web client
//! deserialize them. Keep this crate free of framework dependencies.

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// Opaque identifier used for every entity. Sortable by creation time.
pub type Id = String;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, ToSchema)]
pub struct User {
    pub id: Id,
    pub username: String,
    pub display_name: String,
    pub avatar_url: Option<String>,
    /// True when this identity was created for an agent rather than a person.
    pub bot: bool,
    pub role: Role,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    Admin,
    Member,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, ToSchema)]
pub struct Channel {
    pub id: Id,
    pub name: String,
    pub category_id: Option<Id>,
    pub kind: ChannelKind,
    pub position: i64,
    /// Explicit DM participants. Text channels are visible to all users.
    pub member_ids: Vec<Id>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ChannelKind {
    Text,
    Voice,
    Dm,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, ToSchema)]
pub struct Message {
    pub id: Id,
    pub channel_id: Id,
    pub author_id: Id,
    pub content: String,
    pub reply_to: Option<Id>,
    pub created_at: String,
    pub edited_at: Option<String>,
    pub attachments: Vec<Upload>,
}

/// Every event pushed over the WebSocket stream. The CLI's `tail` prints these.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, ToSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Event {
    MessageCreated(Message),
    MessageEdited(Message),
    MessageDeleted {
        id: Id,
        channel_id: Id,
    },
    Typing {
        channel_id: Id,
        user_id: Id,
    },
    Presence {
        user_id: Id,
        online: bool,
    },
    /// Reconnect or lag requires refetching channel state. No replay is promised.
    Resync {
        reason: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, ToSchema)]
pub struct Health {
    pub ok: bool,
    pub version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ApiError {
    pub error: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct Login {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct Register {
    pub username: String,
    pub password: String,
    pub invite: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct Bootstrap {
    pub username: String,
    pub password: String,
    pub bootstrap_token: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct Session {
    pub token: String,
    pub csrf_token: String,
    pub expires_at: i64,
    pub user: User,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CreateInvite {
    pub uses: u32,
    pub expires_in_hours: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct Invite {
    pub id: Id,
    pub code: String,
    pub uses_left: u32,
    pub expires_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CreateToken {
    pub name: String,
    /// Omit for self; a human may also mint credentials for a bot they own.
    pub user_id: Option<Id>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct Token {
    pub id: Id,
    pub name: String,
    pub user_id: Id,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct TokenSecret {
    pub token: String,
    pub credential: Token,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CreateBot {
    pub username: String,
    pub display_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct BotCreated {
    pub user: User,
    pub credential: TokenSecret,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct Category {
    pub id: Id,
    pub name: String,
    pub position: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct SaveCategory {
    pub name: String,
    pub position: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct SaveChannel {
    pub name: String,
    pub category_id: Option<Id>,
    pub position: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CreateDm {
    pub member_ids: Vec<Id>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CreateMessage {
    pub content: String,
    #[serde(default)]
    pub reply_to: Option<Id>,
    #[serde(default)]
    pub upload_ids: Vec<Id>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct EditMessage {
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, utoipa::IntoParams)]
pub struct MessageQuery {
    pub before: Option<Id>,
    pub after: Option<Id>,
    pub limit: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct BeginUpload {
    pub channel_id: Id,
    pub filename: String,
    pub content_type: String,
    pub size: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, ToSchema)]
pub struct Upload {
    pub id: Id,
    pub channel_id: Id,
    pub filename: String,
    pub content_type: String,
    pub size: i64,
    pub offset: i64,
    pub complete: bool,
}
