use crate::Id;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum DevicePlatform {
    Ios,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct RegisterDevice {
    pub platform: DevicePlatform,
    /// APNs token encoded as hexadecimal. Do not log or expose this credential.
    pub token: String,
    pub app_version: String,
}

/// Registration receipt. Tokens are deliberately omitted from responses.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, ToSchema)]
pub struct Device {
    pub id: Id,
    pub platform: DevicePlatform,
    pub app_version: String,
}

/// An invitation expires 45 seconds after creation. Repeated creation by the
/// same caller returns the existing invitation without notifying again.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, ToSchema)]
pub struct CallInvitation {
    pub channel_id: Id,
    pub from_user_id: Id,
    /// Unix time in seconds. Clients must dismiss an unanswered invitation then.
    pub expires_at: i64,
}

/// Identify the invitation being declined so a delayed request cannot dismiss
/// a newer call in the same DM.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct DeclineCallInvitation {
    pub from_user_id: Id,
    pub expires_at: i64,
}
