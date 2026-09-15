use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use utoipa::ToSchema;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum SoundEvent {
    Message,
    Mention,
    Dm,
    CallJoin,
    CallLeave,
    SomeoneJoined,
    SomeoneLeft,
    ScreenShareStarted,
    TerminalBell,
    UploadComplete,
    Error,
}
impl SoundEvent {
    pub const ALL: [Self; 11] = [
        Self::Message,
        Self::Mention,
        Self::Dm,
        Self::CallJoin,
        Self::CallLeave,
        Self::SomeoneJoined,
        Self::SomeoneLeft,
        Self::ScreenShareStarted,
        Self::TerminalBell,
        Self::UploadComplete,
        Self::Error,
    ];
    pub fn name(self) -> String {
        serde_json::to_value(self)
            .unwrap()
            .as_str()
            .unwrap()
            .to_owned()
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum SoundRef {
    Builtin { name: String },
    Upload { id: String },
    Silent,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct SoundPack {
    pub id: String,
    pub name: String,
    pub sounds: BTreeMap<SoundEvent, SoundRef>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(default, deny_unknown_fields)]
pub struct SoundPreferences {
    pub pack_id: Option<String>,
    pub custom_packs: Vec<SoundPack>,
    pub overrides: BTreeMap<SoundEvent, SoundRef>,
    pub volumes: BTreeMap<SoundEvent, u8>,
    pub master_volume: u8,
}
impl Default for SoundPreferences {
    fn default() -> Self {
        Self {
            pack_id: None,
            custom_packs: vec![],
            overrides: BTreeMap::new(),
            volumes: BTreeMap::new(),
            master_volume: 70,
        }
    }
}
pub fn sound_id(id: &str) -> bool {
    id.as_bytes().first().is_some_and(u8::is_ascii_alphanumeric)
        && id.len() <= 64
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_' || b == b'.')
}
impl SoundPack {
    pub fn validate(&self) -> Result<(), &'static str> {
        if !sound_id(&self.id)
            || self.name.trim().is_empty()
            || self.name.chars().count() > 60
            || self.name.chars().any(char::is_control)
        {
            return Err("Pack needs a safe ID and a name of 1–60 characters");
        }
        for sound in self.sounds.values() {
            validate_ref(sound)?;
        }
        Ok(())
    }
}
fn validate_ref(sound: &SoundRef) -> Result<(), &'static str> {
    match sound {
        SoundRef::Upload { id } if !sound_id(id) => Err("Invalid sound ID"),
        SoundRef::Builtin { name } if !sound_id(name) => Err("Invalid built-in sound name"),
        _ => Ok(()),
    }
}
impl SoundPreferences {
    pub fn validate(&self) -> Result<(), &'static str> {
        if self.custom_packs.len() > 12
            || self.master_volume > 100
            || self.volumes.values().any(|v| *v > 100)
        {
            return Err("Up to 12 packs; volumes must be 0–100");
        }
        let mut ids = std::collections::BTreeSet::new();
        for pack in &self.custom_packs {
            pack.validate()?;
            if !ids.insert(&pack.id) || pack.id == "den" {
                return Err("Duplicate or reserved pack ID");
            }
        }
        for sound in self.overrides.values() {
            validate_ref(sound)?;
        }
        Ok(())
    }
}
pub fn builtin_sound_pack() -> SoundPack {
    SoundPack {
        id: "den".into(),
        name: "Den".into(),
        sounds: SoundEvent::ALL
            .into_iter()
            .map(|event| (event, SoundRef::Builtin { name: event.name() }))
            .collect(),
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct ResolvedSound {
    pub sound: SoundRef,
    pub url: Option<String>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct SoundState {
    pub preferences: SoundPreferences,
    pub server_pack: SoundPack,
    pub resolved: BTreeMap<SoundEvent, ResolvedSound>,
}
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct InstallSound {
    pub event: Option<SoundEvent>,
}
