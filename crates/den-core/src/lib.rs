//! Shared API types. The server serializes these, the CLI and the web client
//! deserialize them. Keep this crate free of framework dependencies.

use serde::{Deserialize, Serialize};

/// Opaque identifier used for every entity. Sortable by creation time.
pub type Id = String;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct User {
    pub id: Id,
    pub username: String,
    pub display_name: String,
    pub avatar_url: Option<String>,
    /// True when this identity was created for an agent rather than a person.
    pub bot: bool,
    pub role: Role,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    Admin,
    Member,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Channel {
    pub id: Id,
    pub name: String,
    pub category_id: Option<Id>,
    pub kind: ChannelKind,
    pub position: i64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ChannelKind {
    Text,
    Voice,
    Dm,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Message {
    pub id: Id,
    pub channel_id: Id,
    pub author_id: Id,
    pub content: String,
    pub reply_to: Option<Id>,
    pub created_at: String,
    pub edited_at: Option<String>,
}

/// Every event pushed over the WebSocket stream. The CLI's `tail` prints these.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Event {
    MessageCreated(Message),
    MessageEdited(Message),
    MessageDeleted { id: Id, channel_id: Id },
    Typing { channel_id: Id, user_id: Id },
    Presence { user_id: Id, online: bool },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Health {
    pub ok: bool,
    pub version: String,
}
