# dot-settings

Owns: the user settings file (typed `Settings`, defaults, TOML load and save, format version) and cloud API keys in the macOS Keychain.

Must not know about: the UI, models, providers, or where the file lives. The caller passes the path; the app uses `~/Library/Application Support/Hey Dot/settings.toml`. Provider ids are opaque strings chosen by the caller.

Entry points: `load_settings(path)`, `save_settings(path, &settings)`, `Settings::default()`; `use_macos_keychain()` once at app start, then `save_api_key` and `read_api_key`.

Invariants and gotchas:
- A missing file loads defaults and writes nothing. Saving is the caller's decision.
- A file that fails to parse, or has a `version` above `CURRENT_SETTINGS_VERSION`, is an error and is never rewritten by this crate. A caller that saves defaults after that error destroys the user's file; show the error instead.
- Adding a field needs no version bump: every field has a default. Bump the version only when a field changes meaning or type, and add the migration in `load_settings` in the same commit.
- Unknown keys are ignored so an older build can read a newer file; saving from the older build drops those keys.
- Saves go through `settings.toml.tmp`, an fsync and a rename, so a crash or power loss never leaves a half-written file.
- `load_settings` and `save_settings` are blocking (the save waits on F_FULLFSYNC). An async caller runs them in `spawn_blocking`.
- Hotkey strings use the global-hotkey format (`Alt+Space`); validation happens where the shortcut is registered.
- API keys never go in `Settings`, the TOML, or any log line. Keychain service is `com.shub3am.heydot`, account is the provider id.
- The credential store is process-global: without `use_macos_keychain()` every key call fails. Tests set `keyring_core::mock::Store` once instead; the real Keychain path is not exercised in CI.

Called by: `app/src-tauri`, which reads `Settings::default()` for the screenshot size (from Phase 1 step 4). Loading the file and the Keychain come with the settings window.
