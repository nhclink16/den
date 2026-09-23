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
    pub light_theme: String,
    pub dark_theme: String,
    #[serde(default)]
    #[schema(schema_with = crate::nullable_object_schema::<Background>)]
    pub background: Option<Background>,
    /// A second wallpaper just for the sidebar. When set, `background` covers the
    /// rest of the app and this one the sidebar; its `scope` is ignored. A client
    /// that omits the field on save keeps the stored one (older apps predate it).
    #[serde(default)]
    #[schema(schema_with = crate::nullable_object_schema::<Background>)]
    pub sidebar_background: Option<Background>,
    #[serde(
        default = "default_contrast",
        deserialize_with = "clamped::<_, 80, 120>"
    )]
    #[schema(minimum = 80, maximum = 120, default = 100)]
    pub contrast: u8,
    pub custom_themes: Vec<Theme>,
}
impl Default for Appearance {
    fn default() -> Self {
        Self {
            mode: AppearanceMode::System,
            light_theme: "den".into(),
            dark_theme: "den".into(),
            background: None,
            sidebar_background: None,
            contrast: 100,
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
        if [&self.light_theme, &self.dark_theme]
            .into_iter()
            .any(|id| !themes.iter().any(|t| &t.id == id))
        {
            return Err("Select an existing theme family");
        }
        Ok(())
    }
}

fn default_contrast() -> u8 {
    100
}

// Accept out-of-range JSON integers, then store only the bounded value.
fn clamped<'de, D: serde::Deserializer<'de>, const MIN: u8, const MAX: u8>(
    deserializer: D,
) -> Result<u8, D::Error> {
    let value = i64::deserialize(deserializer)?;
    Ok(value.clamp(i64::from(MIN), i64::from(MAX)) as u8)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "kebab-case")]
pub enum BackgroundBuiltin {
    // Painted by clients from the active palette. The wallpaper redesign gave
    // these new names, but the wire keeps the old ones: iOS builds that predate
    // it decode this enum strictly and would refuse a name they do not know.
    // The new names are accepted as aliases and clients show them.
    #[serde(rename = "ember-sky", alias = "lamplight")]
    Lamplight,
    #[serde(rename = "harbor", alias = "doorway")]
    Doorway,
    #[serde(rename = "dunes", alias = "contours")]
    Contours,
    #[serde(rename = "slate-mist", alias = "plaid")]
    Plaid,
    #[serde(rename = "aurora", alias = "clearing")]
    Clearing,
    #[serde(rename = "grain", alias = "paper")]
    Paper,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum BackgroundSource {
    Builtin { name: BackgroundBuiltin },
    Upload { id: String },
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum BackgroundScope {
    App,
    Sidebar,
    Chat,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum BackgroundFit {
    Cover,
    Contain,
    Tile,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct Background {
    pub source: BackgroundSource,
    #[serde(deserialize_with = "clamped::<_, 0, 40>")]
    #[schema(minimum = 0, maximum = 40)]
    pub blur: u8,
    #[serde(deserialize_with = "clamped::<_, 0, 80>")]
    #[schema(minimum = 0, maximum = 80)]
    pub dim: u8,
    #[serde(deserialize_with = "clamped::<_, 50, 150>")]
    #[schema(minimum = 50, maximum = 150)]
    pub saturate: u8,
    pub scope: BackgroundScope,
    pub fit: BackgroundFit,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
pub struct BackgroundImage {
    /// Opaque SHA-256 content ID. Changes when the image bytes change.
    pub id: String,
    pub content_type: String,
    pub size: u64,
    pub width: u32,
    pub height: u32,
    /// Unix seconds when it was uploaded. Absent from servers before the library.
    #[serde(default)]
    pub uploaded_at: i64,
}
