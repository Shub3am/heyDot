# dot-settings

Owns: the user settings file (typed `Settings`, defaults, TOML load and save, format version).

Must not know about: the UI, models, providers, or where the file lives. The caller passes the path; the app uses `~/Library/Application Support/Hey Dot/settings.toml`.

Entry points: `load_settings(path)`, `save_settings(path, &settings)`, `Settings::default()`.

Invariants and gotchas:
- A missing file loads defaults and writes nothing. Saving is the caller's decision.
- A file that fails to parse, or has a `version` above `CURRENT_SETTINGS_VERSION`, is an error and is never rewritten by this crate. A caller that saves defaults after that error destroys the user's file; show the error instead.
- Adding a field needs no version bump: every field has a default. Bump the version only when a field changes meaning or type, and add the migration in `load_settings` in the same commit.
- Unknown keys are ignored so an older build can read a newer file; saving from the older build drops those keys.
- Saves go through `settings.toml.tmp` and a rename, so a crash never leaves a half-written file.
- Hotkey strings use the global-hotkey format (`Alt+Space`); validation happens where the shortcut is registered.

Called by: `app/src-tauri` (from Phase 1 step 3 on).
