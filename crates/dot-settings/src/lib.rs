//! Hey Dot's user settings file and the API keys that must never be in it.
//! Must not know about the UI, models or providers, or where the settings file lives.

mod settings_file;

pub use settings_file::{
    CURRENT_SETTINGS_VERSION, Settings, SettingsError, TalkMode, load_settings, save_settings,
};
