use std::fs;

use dot_settings::{
    CURRENT_SETTINGS_VERSION, Settings, SettingsError, TalkMode, load_settings, save_settings,
};

#[test]
fn missing_file_loads_defaults() {
    let settings_dir = tempfile::tempdir().unwrap();

    let settings = load_settings(&settings_dir.path().join("settings.toml")).unwrap();

    assert_eq!(settings, Settings::default());
    assert_eq!(settings.version, CURRENT_SETTINGS_VERSION);
    assert_eq!(settings.talk_mode, TalkMode::Hold);
    assert_eq!(settings.talk_hotkey, "Alt+Space");
    assert_eq!(settings.chat_panel_hotkey, "Alt+Shift+Space");
    assert_eq!(settings.end_of_speech_silence_ms, 800);
    assert_eq!(settings.screenshot_max_edge_px, 1600);
    assert!(!settings.launch_at_login);
}

#[test]
fn saved_settings_load_back_unchanged() {
    let settings_dir = tempfile::tempdir().unwrap();
    let settings_path = settings_dir.path().join("Hey Dot/settings.toml");
    let settings = Settings {
        talk_mode: TalkMode::Tap,
        end_of_speech_silence_ms: 1200,
        ..Settings::default()
    };

    save_settings(&settings_path, &settings).unwrap();

    assert_eq!(load_settings(&settings_path).unwrap(), settings);
    assert!(!settings_path.with_extension("toml.tmp").exists());
}

#[test]
fn keys_missing_from_the_file_take_their_defaults() {
    let settings_dir = tempfile::tempdir().unwrap();
    let settings_path = settings_dir.path().join("settings.toml");
    fs::write(&settings_path, "talk_mode = \"tap\"\n").unwrap();

    let settings = load_settings(&settings_path).unwrap();

    assert_eq!(
        settings,
        Settings {
            talk_mode: TalkMode::Tap,
            ..Settings::default()
        }
    );
}

#[test]
fn unknown_keys_are_ignored() {
    let settings_dir = tempfile::tempdir().unwrap();
    let settings_path = settings_dir.path().join("settings.toml");
    fs::write(
        &settings_path,
        "setting_from_a_later_build = true\nscreenshot_max_edge_px = 1200\n",
    )
    .unwrap();

    let settings = load_settings(&settings_path).unwrap();

    assert_eq!(settings.screenshot_max_edge_px, 1200);
}

#[test]
fn file_from_a_newer_version_is_rejected_and_left_untouched() {
    let settings_dir = tempfile::tempdir().unwrap();
    let settings_path = settings_dir.path().join("settings.toml");
    let newer_file = format!("version = {}\n", CURRENT_SETTINGS_VERSION + 1);
    fs::write(&settings_path, &newer_file).unwrap();

    let error = load_settings(&settings_path).unwrap_err();

    assert!(matches!(
        error,
        SettingsError::NewerVersion { found, supported }
            if found == CURRENT_SETTINGS_VERSION + 1 && supported == CURRENT_SETTINGS_VERSION
    ));
    assert_eq!(fs::read_to_string(&settings_path).unwrap(), newer_file);
}

#[test]
fn invalid_value_is_an_error_naming_the_key() {
    let settings_dir = tempfile::tempdir().unwrap();
    let settings_path = settings_dir.path().join("settings.toml");
    fs::write(&settings_path, "talk_mode = \"shout\"\n").unwrap();

    let error = load_settings(&settings_path).unwrap_err();

    assert!(matches!(error, SettingsError::Invalid(_)));
    assert!(error.to_string().contains("talk_mode"), "{error}");
    assert_eq!(
        fs::read_to_string(&settings_path).unwrap(),
        "talk_mode = \"shout\"\n"
    );
}
