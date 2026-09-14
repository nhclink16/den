use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ThemeAppearance {
    Light,
    Dark,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum AppearanceMode {
    Light,
    Dark,
    System,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ThemeRadius {
    Sharp,
    Soft,
    Round,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ThemeDensity {
    Compact,
    Comfortable,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct ThemeColors {
    pub bg: String,
    pub bg2: String,
    pub bg3: String,
    pub line: String,
    pub ink: String,
    pub ink2: String,
    pub ink3: String,
    pub accent: String,
    pub success: String,
    pub danger: String,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub generated: bool,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct ThemeFonts {
    pub display: String,
    pub body: String,
    pub mono: String,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct Theme {
    pub id: String,
    pub name: String,
    pub light: ThemeColors,
    pub dark: ThemeColors,
    pub fonts: ThemeFonts,
    pub radius: ThemeRadius,
    pub density: ThemeDensity,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct Appearance {
    pub mode: AppearanceMode,
    pub theme: String,
    pub custom_themes: Vec<Theme>,
}
impl Default for Appearance {
    fn default() -> Self {
        Self {
            mode: AppearanceMode::System,
            theme: "den".into(),
            custom_themes: vec![],
        }
    }
}
pub fn builtin_themes() -> Vec<Theme> {
    serde_json::from_str(include_str!("themes.json")).expect("validated built-in themes")
}
impl Theme {
    pub fn validate(&self) -> Result<(), &'static str> {
        let name = |v: &str| {
            !v.trim().is_empty()
                && v.chars().count() <= 40
                && v.chars()
                    .all(|c| c.is_alphanumeric() || matches!(c, ' ' | '-' | '_'))
        };
        if [
            &self.id,
            &self.name,
            &self.fonts.display,
            &self.fonts.body,
            &self.fonts.mono,
        ]
        .into_iter()
        .any(|v| !name(v))
        {
            return Err("Names must be 1-40 letters, numbers, spaces, hyphens or underscores");
        }
        for c in [&self.light, &self.dark] {
            if [
                &c.bg, &c.bg2, &c.bg3, &c.line, &c.ink, &c.ink2, &c.ink3, &c.accent, &c.success,
                &c.danger,
            ]
            .into_iter()
            .any(|v| {
                v.len() != 7
                    || !v.starts_with('#')
                    || !v.as_bytes()[1..].iter().all(u8::is_ascii_hexdigit)
            }) {
                return Err("Colors must be six-digit hex values");
            }
        }
        Ok(())
    }
}
impl Appearance {
    pub fn validate(&self) -> Result<(), &'static str> {
        if self.custom_themes.len() > 12
            || serde_json::to_vec(&self.custom_themes)
                .map_err(|_| "Invalid themes")?
                .len()
                > 16 * 1024
        {
            return Err("Use at most 12 custom themes and 16 KiB total");
        }
        let mut themes = builtin_themes();
        for t in &self.custom_themes {
            t.validate()?;
            if themes.iter().any(|v| v.id == t.id) {
                return Err("Theme IDs must be unique and cannot replace built-ins");
            }
            themes.push(t.clone());
        }
        if !themes.iter().any(|t| t.id == self.theme) {
            return Err("Select an existing theme family");
        }
        Ok(())
    }
}
