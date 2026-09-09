//! Hey Dot's user settings file and the secrets that must never be in it: API keys and the saved-chat key.
//! Must not know about the UI, models or providers, or where the settings file lives.

mod api_keys;
mod history_key;
mod settings_file;

pub use api_keys::{ApiKeyError, read_api_key, save_api_key, use_macos_keychain};
pub use history_key::read_or_create_history_key;
pub use settings_file::{
    CURRENT_SETTINGS_VERSION, Settings, SettingsError, TalkMode, load_settings, save_settings,
};
