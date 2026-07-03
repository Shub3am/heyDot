//! Hey Dot's user settings file and the API keys that must never be in it.
//! Must not know about the UI, models or providers, or where the settings file lives.

mod api_keys;
mod settings_file;

pub use api_keys::{ApiKeyError, read_api_key, save_api_key, use_macos_keychain};
pub use settings_file::{
    CURRENT_SETTINGS_VERSION, Settings, SettingsError, TalkMode, load_settings, save_settings,
};
