use crate::Id;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum DevicePlatform {
    Ios,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum DevicePurpose {
    #[default]
    Alert,
    Voip,
}
impl DevicePurpose {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Alert => "alert",
            Self::Voip => "voip",
        }
    }
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum DeviceEnvironment {
    #[default]
    Sandbox,
    Production,
}
impl DeviceEnvironment {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Sandbox => "sandbox",
            Self::Production => "production",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct RegisterDevice {
    pub platform: DevicePlatform,
    /// APNs token encoded as hexadecimal. Do not log or expose this credential.
    pub token: String,
    pub app_version: String,
    #[serde(default)]
    pub purpose: DevicePurpose,
    /// Defaults to DEN_APNS_ENV. Explicit sandbox and production route independently.
    #[serde(default)]
    #[schema(value_type = Option<String>)]
    pub environment: Option<DeviceEnvironment>,
    /// Installation UUID; required for PushKit, optional for legacy alert clients.
    #[serde(default)]
    pub client_id: Option<String>,
}

/// Registration receipt. Tokens are deliberately omitted from responses.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, ToSchema)]
pub struct Device {
    pub id: Id,
    pub platform: DevicePlatform,
    pub app_version: String,
    pub purpose: DevicePurpose,
    pub environment: DeviceEnvironment,
    pub client_id: Option<String>,
}

/// An invitation expires 45 seconds after creation. Repeated creation by the
/// same caller returns the existing invitation without notifying again.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, ToSchema)]
pub struct CallInvitation {
    pub id: Id,
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
    #[serde(default)]
    #[schema(value_type = Option<String>)]
    pub invitation_id: Option<Id>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum InvitationStatus {
    Ringing,
    Active,
    Cancelled,
    Expired,
    Ended,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, ToSchema)]
pub struct CallAcceptance {
    pub user_id: Id,
    pub answer_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, ToSchema)]
pub struct CallInvitationState {
    pub invitation: CallInvitation,
    pub state: InvitationStatus,
    pub accepted: Vec<CallAcceptance>,
    pub declined_user_ids: Vec<Id>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct AcceptCallInvitation {
    pub invitation_id: Id,
    pub answer_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct IdentifyCallInvitation {
    pub invitation_id: Id,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct RedeemCallInvitation {
    /// One-use read capability. Never place in a URL or log.
    pub ticket: String,
}

/// Generated projection of the incoming VoIP custom fields, excluding aps/type.
/// Report these to CallKit immediately, before any network request.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct IncomingVoipCall {
    pub invitation_id: Id,
    pub channel_id: Id,
    pub from_user_id: Id,
    pub from_display_name: String,
    pub expires_at: i64,
    /// One-use state-only read capability, never a media or login credential.
    pub fetch_ticket: String,
}
