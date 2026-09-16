//! Thread contract. A thread is a conversation hanging off one root message in a
//! channel; it is resolved, never archived, and carries no access of its own.

use crate::Id;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// Shared thread metadata. Deliberately free of per-user state so one payload can
/// be broadcast to every member who can see the parent channel.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, ToSchema)]
pub struct ThreadSummary {
    pub id: Id,
    pub channel_id: Id,
    /// The message the conversation hangs off. It stays in the main conversation.
    pub root_message_id: Id,
    /// A snapshot inferred at creation. Editing the root never rewrites it.
    pub title: String,
    pub created_by: Id,
    pub created_at: String,
    /// Both resolution fields are set together or not at all.
    pub resolved_at: Option<String>,
    #[schema(value_type = Option<String>)]
    pub resolved_by: Option<Id>,
    pub reply_count: i64,
    #[schema(value_type = Option<String>)]
    pub last_reply_id: Option<Id>,
    pub last_activity_at: String,
}

/// One user's position in one thread. Following changes unread accounting, never access.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, ToSchema)]
pub struct ThreadReadState {
    pub thread_id: Id,
    pub channel_id: Id,
    #[schema(value_type = Option<String>)]
    pub last_read_id: Option<Id>,
    pub following: bool,
    pub unread_count: i64,
    pub mention_count: i64,
    /// Unread replies eligible under current preferences, excluding own messages.
    pub notification_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, ToSchema)]
pub struct ThreadView {
    pub thread: ThreadSummary,
    pub read_state: ThreadReadState,
}

/// Thread IDs are creation-ordered ULIDs, so listing pages by ID like the other lists.
/// Filters combine: a thread must satisfy every one that is present.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, utoipa::IntoParams)]
#[into_params(parameter_in = Query)]
pub struct ThreadQuery {
    /// Absent lists every thread; false lists the open strip; true lists resolved history.
    pub resolved: Option<bool>,
    /// True lists only threads contributing unread activity to this caller's channel
    /// total. Absent or false applies no unread filter.
    pub unread_only: Option<bool>,
    #[schema(value_type = Option<String>)]
    #[param(value_type = Option<String>)]
    pub before: Option<Id>,
    pub limit: Option<u32>,
}

/// Advances one thread's own read position. A thread read never moves the main
/// conversation's position, and reading does not by itself follow the thread.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct MarkThreadRead {
    pub message_id: Id,
}

/// Rename, resolve or reopen. An omitted field is left unchanged.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct UpdateThread {
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub resolved: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct FollowThread {
    pub following: bool,
}
