mod profile;
pub use profile::*;
mod appearance;
pub use appearance::*;
mod host;
pub use host::*;
mod ios;
pub use ios::*;
// Shared API types. The server serializes these, the CLI and the web client
// deserialize them. Keep this crate free of framework dependencies.

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

// Apple's Swift generator rejects a standalone null branch in oneOf. Keep the
// derived object fields, but express nullability as an OpenAPI 3.1 type array.
fn nullable_object_schema<T: utoipa::PartialSchema>() -> utoipa::openapi::schema::Object {
    use utoipa::openapi::{
        schema::{Schema, SchemaType, Type},
        RefOr,
    };

    let RefOr::T(Schema::Object(mut schema)) = T::schema() else {
        unreachable!("nullable_object_schema requires a derived object schema");
    };
    schema.schema_type = SchemaType::Array(vec![Type::Object, Type::Null]);
    schema
}

/// Opaque identifier used for every entity. Sortable by creation time.
pub type Id = String;

/// A short-lived, single-use credential for a native client's WebSocket upgrade.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct WsTicket {
    pub ticket: String,
    pub expires_in: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct Instance {
    pub instance_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon_url: Option<String>,
    pub version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, ToSchema)]
pub struct User {
    pub id: Id,
    pub username: String,
    pub display_name: String,
    pub avatar_url: Option<String>,
    pub banner_url: Option<String>,
    pub bio: Option<String>,
    pub accent: Option<String>,
    #[schema(schema_with = nullable_object_schema::<Status>)]
    pub status: Option<Status>,
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
    #[schema(value_type = Option<String>)]
    pub category_id: Option<Id>,
    pub kind: ChannelKind,
    pub position: i64,
    /// Explicit DM participants. Text and voice channels are visible to all users.
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
    #[schema(value_type = Option<String>)]
    pub reply_to: Option<Id>,
    pub created_at: String,
    pub edited_at: Option<String>,
    pub attachments: Vec<Upload>,
    #[serde(default)]
    pub objects: Vec<ObjectSummary>,
    #[serde(default)]
    pub reactions: Vec<Reaction>,
    #[serde(default)]
    pub mention_ids: Vec<Id>,
}

/// Every event pushed over the WebSocket stream. The CLI's `tail` prints these.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, ToSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Event {
    UserUpdated {
        user: User,
    },
    AppearanceUpdated {
        user_id: String,
        appearance: Appearance,
    },
    TerminalOutput {
        session_id: Id,
        bytes: Vec<u8>,
        #[schema(value_type = Option<String>)]
        connection_id: Option<Id>,
    },
    TerminalState {
        session: TerminalState,
    },
    AccessDecided {
        user_id: Id,
        request: AccessRequest,
    },
    ObjectPatched {
        id: Id,
        channel_id: Id,
        version: i64,
        put: Vec<serde_json::Value>,
        remove: Vec<Id>,
        author_id: Id,
    },
    ObjectPresence {
        id: Id,
        channel_id: Id,
        user_ids: Vec<Id>,
    },
    ObjectCursor {
        id: Id,
        user_id: Id,
        x: f64,
        y: f64,
        page_id: String,
    },
    SettingsUpdated {
        settings: Settings,
    },
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
    CallState {
        channel_id: Id,
        participant_ids: Vec<Id>,
    },
    /// Only other DM members with a pending, unexpired invitation receive this.
    CallInvite {
        invitation_id: Id,
        channel_id: Id,
        from_user_id: Id,
        expires_at: i64,
    },
    CallInvitationState {
        call: CallInvitationState,
    },
    CallInviteAccepted {
        call: CallInvitationState,
        user_id: Id,
        answer_id: String,
    },
    CallInviteCancelled {
        call: CallInvitationState,
    },
    CallInviteExpired {
        call: CallInvitationState,
    },
    CallEnded {
        call: CallInvitationState,
    },
    Presence {
        user_id: Id,
        online: bool,
    },
    ReactionsUpdated {
        message_id: Id,
        channel_id: Id,
        reactions: Vec<Reaction>,
    },
    ReadStateUpdated {
        user_id: Id,
        state: ChannelReadState,
    },
    NotificationPreferencesUpdated {
        user_id: Id,
        preferences: NotificationPreferences,
    },
    Notification {
        user_id: Id,
        message: Message,
        reason: NotificationReason,
    },
    /// Reconnect or lag requires refetching channel state, presence, and calls. No replay is promised.
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
    #[schema(value_type = Option<String>)]
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
    #[schema(value_type = Option<String>)]
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
    #[schema(value_type = Option<String>)]
    pub reply_to: Option<Id>,
    #[serde(default)]
    pub upload_ids: Vec<Id>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct EditMessage {
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, utoipa::IntoParams)]
#[into_params(parameter_in = Query)]
pub struct MessageQuery {
    #[schema(value_type = Option<String>)]
    #[param(value_type = Option<String>)]
    pub before: Option<Id>,
    #[schema(value_type = Option<String>)]
    #[param(value_type = Option<String>)]
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
    /// Authenticated PNG preview, at most 512 by 512. None when unavailable.
    pub thumbnail_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, ToSchema)]
pub struct Reaction {
    pub emoji: String,
    pub user_ids: Vec<Id>,
}
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct SetReaction {
    pub emoji: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct MarkRead {
    pub message_id: Id,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, ToSchema)]
pub struct ChannelReadState {
    pub channel_id: Id,
    #[schema(value_type = Option<String>)]
    pub last_read_id: Option<Id>,
    pub unread_count: i64,
    pub mention_count: i64,
    /// Unread messages eligible under current preferences, excluding own messages.
    pub notification_count: i64,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, ToSchema)]
pub struct NotificationPreferences {
    pub mentions: bool,
    pub dms: bool,
    pub subscribed_channel_ids: Vec<Id>,
}
impl Default for NotificationPreferences {
    fn default() -> Self {
        Self {
            mentions: true,
            dms: true,
            subscribed_channel_ids: Vec::new(),
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum NotificationReason {
    Mention,
    Dm,
    SubscribedChannel,
}
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, utoipa::IntoParams)]
#[into_params(parameter_in = Query)]
pub struct SearchMessages {
    pub q: String,
    #[schema(value_type = Option<String>)]
    #[param(value_type = Option<String>)]
    pub channel_id: Option<Id>,
    #[schema(value_type = Option<String>)]
    #[param(value_type = Option<String>)]
    pub before: Option<Id>,
    pub limit: Option<u32>,
}
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum ClientEvent {
    ObjectOpen {
        object_id: Id,
    },
    ObjectClose {
        object_id: Id,
    },
    ObjectCursor {
        object_id: Id,
        x: f64,
        y: f64,
        page_id: String,
    },
    Typing {
        channel_id: Id,
    },
}
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct PresenceState {
    pub objects: Vec<ObjectPresence>,
    pub online_user_ids: Vec<Id>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CallToken {
    pub url: String,
    pub token: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, ToSchema)]
pub struct CallState {
    pub channel_id: Id,
    pub participant_ids: Vec<Id>,
}

/// Document records keyed by record id. The server treats record contents as opaque.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, ToSchema)]
pub struct ObjectSummary {
    pub id: Id,
    pub channel_id: Id,
    #[schema(value_type = Option<String>)]
    pub message_id: Option<Id>,
    pub kind: String,
    pub name: String,
    pub version: i64,
    #[schema(value_type = Option<String>)]
    pub thumbnail_upload_id: Option<Id>,
    pub thumbnail_url: Option<String>,
    pub created_by: Id,
    pub created_at: String,
    pub updated_at: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct Object {
    #[serde(flatten)]
    pub summary: ObjectSummary,
    pub state: std::collections::BTreeMap<String, serde_json::Value>,
}
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CreateObject {
    pub kind: String,
    pub name: String,
    #[serde(default)]
    pub state: std::collections::BTreeMap<String, serde_json::Value>,
}
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ObjectPatch {
    pub base_version: i64,
    #[serde(default)]
    pub put: Vec<serde_json::Value>,
    #[serde(default)]
    pub remove: Vec<Id>,
}
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ObjectVersion {
    pub version: i64,
}
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct UpdateObject {
    pub name: Option<String>,
    #[schema(value_type = Option<String>)]
    pub thumbnail_upload_id: Option<Id>,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, ToSchema)]
pub struct Settings {
    pub canvas_enabled: bool,
    pub instance_name: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct UpdateSettings {
    pub canvas_enabled: Option<bool>,
    pub instance_name: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ObjectPresence {
    pub id: Id,
    pub channel_id: Id,
    pub user_ids: Vec<Id>,
}

pub mod theme_color;
