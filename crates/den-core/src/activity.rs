use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::Id;

/// What someone is doing, as a verb the client puts in front of the name.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ActivityKind {
    Playing,
    Listening,
    Watching,
    Using,
    Working,
}

/// One thing a person is doing right now. A person can have several at once, one
/// per slot: the desktop app, Spotify, the Minecraft relay, an agent's CLI. Every
/// activity expires unless its source refreshes it, so a crashed source never
/// leaves someone "playing" forever.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, ToSchema)]
pub struct Activity {
    /// Who set it: `desktop`, `spotify`, `minecraft`, `cli`, or another source's name.
    pub slot: String,
    pub kind: ActivityKind,
    /// The game, app, track or task.
    pub name: String,
    /// A second line: the artist, a server, a file type. Never a window title.
    pub details: Option<String>,
    /// Artwork for the activity. Only the server sets this (Spotify album art).
    pub image_url: Option<String>,
    /// Unix milliseconds when this activity began, kept across refreshes.
    pub started_at: i64,
    /// Unix milliseconds after which the activity is gone unless refreshed.
    pub expires_at: i64,
}

/// Body for `PUT /users/{id}/activities/{slot}`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct SetActivity {
    pub kind: ActivityKind,
    #[schema(min_length = 1, max_length = 64)]
    pub name: String,
    #[serde(default)]
    #[schema(max_length = 128)]
    pub details: Option<String>,
    /// How long it lasts without a refresh. Defaults to 90 seconds, at most a day.
    #[serde(default)]
    #[schema(minimum = 10, maximum = 86400)]
    pub ttl_seconds: Option<u32>,
}

pub const ACTIVITY_DEFAULT_TTL: u32 = 90;
pub const ACTIVITY_MAX_TTL: u32 = 86_400;
/// Slots the server fills itself. Clients cannot write them.
pub const ACTIVITY_SERVER_SLOTS: &[&str] = &["spotify"];

impl SetActivity {
    /// Trims text and rejects what cannot be shown on one line of a profile card.
    pub fn validate(&mut self) -> Result<(), &'static str> {
        self.name = self.name.trim().to_string();
        let count = self.name.chars().count();
        if count == 0 || count > 64 || self.name.chars().any(char::is_control) {
            return Err("Activity name must be 1-64 characters on one line.");
        }
        if let Some(details) = &self.details {
            let details = details.trim().to_string();
            if details.chars().count() > 128 || details.chars().any(char::is_control) {
                return Err("Activity details must be at most 128 characters on one line.");
            }
            self.details = (!details.is_empty()).then_some(details);
        }
        if let Some(ttl) = self.ttl_seconds {
            if !(10..=ACTIVITY_MAX_TTL).contains(&ttl) {
                return Err("ttl_seconds must be between 10 and 86400.");
            }
        }
        Ok(())
    }
}

/// A slot name: lowercase letters, digits and hyphens, starting with a letter or digit.
pub fn valid_activity_slot(slot: &str) -> bool {
    let bytes = slot.as_bytes();
    !bytes.is_empty()
        && bytes.len() <= 32
        && bytes[0].is_ascii_alphanumeric()
        && bytes
            .iter()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || *b == b'-')
}

/// Everything one person is doing, newest first.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, ToSchema)]
pub struct UserActivities {
    pub user_id: Id,
    pub activities: Vec<Activity>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slots_are_short_lowercase_names() {
        for ok in ["desktop", "cli", "minecraft", "a", "build-2"] {
            assert!(valid_activity_slot(ok), "{ok}");
        }
        for bad in ["", "-x", "Desktop", "a b", "x/y", &"a".repeat(33)] {
            assert!(!valid_activity_slot(bad), "{bad}");
        }
    }

    #[test]
    fn text_is_trimmed_and_kept_to_one_line() {
        let mut ok = SetActivity {
            kind: ActivityKind::Playing,
            name: "  Minecraft ".into(),
            details: Some("  ".into()),
            ttl_seconds: None,
        };
        ok.validate().unwrap();
        assert_eq!(ok.name, "Minecraft");
        assert_eq!(ok.details, None);

        let two_lines = SetActivity {
            name: "a\nb".into(),
            ..ok.clone()
        };
        assert!(two_lines.clone().validate().is_err());
        let forever = SetActivity {
            ttl_seconds: Some(ACTIVITY_MAX_TTL + 1),
            ..ok.clone()
        };
        assert!(forever.clone().validate().is_err());
    }
}
