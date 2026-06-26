//! The settings file: its typed shape, its defaults, and reading and writing it as TOML.
//! Must not hold secrets (API keys live in the Keychain) and must not decide where the
//! file lives; callers pass the path.

use std::io::ErrorKind;
use std::path::Path;

use serde::{Deserialize, Serialize};

/// Bumped only when a field changes meaning or shape. Adding a field does not bump it:
/// `#[serde(default)]` fills fields an older file lacks.
pub const CURRENT_SETTINGS_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub version: u32,
    pub talk_mode: TalkMode,
    pub talk_hotkey: String,
    pub chat_panel_hotkey: String,
    pub end_of_speech_silence_ms: u32,
    pub screenshot_max_edge_px: u32,
    pub launch_at_login: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TalkMode {
    Hold,
    Tap,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            version: CURRENT_SETTINGS_VERSION,
            talk_mode: TalkMode::Hold,
            talk_hotkey: "Alt+Space".to_string(),
            chat_panel_hotkey: "Alt+Shift+Space".to_string(),
            end_of_speech_silence_ms: 800,
            screenshot_max_edge_px: 1600,
            launch_at_login: false,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum SettingsError {
    #[error("could not read or write the settings file: {0}")]
    Io(#[from] std::io::Error),
    #[error("the settings file is not valid: {0}")]
    Invalid(#[from] toml::de::Error),
    #[error("could not encode the settings: {0}")]
    Encode(#[from] toml::ser::Error),
    #[error(
        "the settings file was written by a newer Hey Dot (version {found}, this build reads up to {supported})"
    )]
    NewerVersion { found: u32, supported: u32 },
}

pub fn load_settings(settings_path: &Path) -> Result<Settings, SettingsError> {
    let settings_toml = match std::fs::read_to_string(settings_path) {
        Ok(settings_toml) => settings_toml,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(Settings::default()),
        Err(error) => return Err(error.into()),
    };
    let settings: Settings = toml::from_str(&settings_toml)?;
    if settings.version > CURRENT_SETTINGS_VERSION {
        return Err(SettingsError::NewerVersion {
            found: settings.version,
            supported: CURRENT_SETTINGS_VERSION,
        });
    }
    Ok(settings)
}

pub fn save_settings(settings_path: &Path, settings: &Settings) -> Result<(), SettingsError> {
    let settings_toml = toml::to_string_pretty(settings)?;
    if let Some(settings_dir) = settings_path.parent() {
        std::fs::create_dir_all(settings_dir)?;
    }
    // Write beside the target and rename, so a crash mid-write never leaves a truncated file.
    let staging_path = settings_path.with_extension("toml.tmp");
    std::fs::write(&staging_path, settings_toml)?;
    std::fs::rename(&staging_path, settings_path)?;
    Ok(())
}
