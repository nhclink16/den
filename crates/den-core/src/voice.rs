use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use utoipa::ToSchema;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(default, deny_unknown_fields)]
pub struct MicrophoneSettings {
    pub gain: f64,
    pub echo_cancellation: bool,
    pub noise_suppression: bool,
    pub auto_gain_control: bool,
}
impl Default for MicrophoneSettings {
    fn default() -> Self {
        Self {
            gain: 1.0,
            echo_cancellation: true,
            noise_suppression: true,
            auto_gain_control: true,
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema, Default)]
pub enum CameraResolution {
    #[default]
    #[serde(rename = "auto")]
    Auto,
    #[serde(rename = "720p")]
    Hd,
    #[serde(rename = "1080p")]
    FullHd,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(default, deny_unknown_fields)]
pub struct CameraSettings {
    pub resolution: CameraResolution,
    pub frame_rate: u32,
    pub mirror: bool,
    pub brightness: Option<f64>,
    pub contrast: Option<f64>,
    pub saturation: Option<f64>,
}
impl Default for CameraSettings {
    fn default() -> Self {
        Self {
            resolution: CameraResolution::Auto,
            frame_rate: 30,
            mirror: true,
            brightness: None,
            contrast: None,
            saturation: None,
        }
    }
}
/// PUT merges device entries, preserving other devices on the account.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema, Default)]
#[serde(default, deny_unknown_fields)]
pub struct VoicePreferences {
    pub microphones: BTreeMap<String, MicrophoneSettings>,
    pub cameras: BTreeMap<String, CameraSettings>,
}
impl VoicePreferences {
    pub fn validate(&self) -> Result<(), &'static str> {
        if self.microphones.len() > 64
            || self.cameras.len() > 64
            || self
                .microphones
                .keys()
                .chain(self.cameras.keys())
                .any(|id| id.is_empty() || id.len() > 256)
        {
            return Err("Too many devices or invalid device ID");
        }
        if self
            .microphones
            .values()
            .any(|m| !m.gain.is_finite() || !(0.0..=2.0).contains(&m.gain))
        {
            return Err("Microphone gain must be between 0 and 2");
        }
        if self.cameras.values().any(|c| {
            ![24, 30, 60].contains(&c.frame_rate)
                || [c.brightness, c.contrast, c.saturation]
                    .into_iter()
                    .flatten()
                    .any(|v| !v.is_finite() || v.abs() > 1_000_000.0)
        }) {
            return Err("Invalid camera settings");
        }
        Ok(())
    }
}
