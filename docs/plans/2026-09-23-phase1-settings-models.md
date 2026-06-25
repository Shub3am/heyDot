# Phase 1 step 2: dot-settings and dot-models Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Two pure-logic Rust crates: `dot-settings` (typed TOML settings file plus API keys in the macOS Keychain) and `dot-models` (pinned model catalog, hardware-based recommendation, resumable sha256-verified downloads).

**Architecture:** Both crates live under `crates/` and join the root Cargo workspace. Neither knows where Hey Dot's folders are: callers pass the settings file path and the models folder, so the app (wired in step 3) owns `~/Library/Application Support/Hey Dot/`. Downloads are async (tokio + reqwest streaming), write to `<name>.part`, verify sha256 from disk, then rename, so a final file name on disk always means "verified".

**Tech Stack:** Rust 1.98 edition 2024; `serde` 1.0.229, `toml` 1.1.6, `thiserror` 2.0.20, `keyring-core` 1.0.0 (+ its `mock` store in tests), `apple-native-keyring-store` 1 with feature `keychain`, `sysinfo` 0.39.6 (`default-features = false`, `features = ["system"]`), `reqwest` 0.13.5 (`stream`), `futures-util` 0.3.34, `sha2` 0.11.0, `hex` 0.4.3, `tokio` 1.53.1; tests: `tempfile` 3.27.0, `axum` 0.8.9, `tower-http` 0.7.1 (`fs`). All APIs used below were compile-checked on 2026-09-23 in a scratch crate (keyring mock persistence across entries, HF Range 206 through the redirect, ServeDir 206, axum middleware on a fallback service, `hex::encode` of a sha2 0.11 digest, global-hotkey parsing `Alt+Space`).

**Spec:** `docs/specs/2026-09-23-phase0-phase1-design.md` (sections "dot-models", "dot-settings", "Testing", build order step 2).

## Global Constraints

- Crate names are final: `dot-settings`, `dot-models`. Workspace `members = ["app/src-tauri", "crates/*"]`.
- Settings: "TOML at `~/Library/Application Support/Hey Dot/settings.toml`, typed struct with defaults, versioned for migration."
- Secrets: "API keys only in the macOS Keychain via `keyring`; never written to the TOML or logs." (`keyring` 4 is a facade over `keyring-core`; this plan uses `keyring-core` directly because only it exposes the mock store.)
- Catalog: "model id, display name, files (HuggingFace URL, sha256, bytes), min RAM, tier."
- Recommend: "Apple Silicon 16 GB+ -> 4B, 32 GB+ offers 8B, Apple Silicon 8 GB -> 2B with quality note, Intel -> recommend cloud, local 2B allowed."
- Downloads: "HTTP range resume, progress events, sha256 verify, atomic rename. A failed hash deletes the partial file and reports it."
- Tests: "`dot-models` download: range resume, hash mismatch, atomic rename, against a local file server."
- Every source file starts with a `//!` header saying why it exists and what it must not do; it never lists functions.
- Every crate has an `AGENTS.md`: owns, must not know about, entry points, invariants and gotchas, called by.
- `pnpm -C app build` must have run once before any workspace-wide cargo command (`generate_context!` reads `app/dist`). Per-crate commands (`cargo test -p dot-models`) do not need it.
- No em dashes in any file. Commits: one logical change each, no co-author lines, nothing pushed until the user has seen test output.

## Where this plan narrows the spec

- **Kokoro is not in the catalog yet.** Its file set (fp32 vs int8 model, per-voice `.bin` vs combined `voices-v1.0.bin`) depends on the G2P decision the spec makes the first dot-tts step. The dot-tts step adds its entry.
- **No `tier` field.** Only chat models have tiers and `recommend` is the only consumer, so each tier is a named static (`QWEN3_VL_2B/4B/8B`) and `recommend` returns `&'static Model`. `min_ram_gb` stays a field and supplies the 16/32 GiB thresholds.
- **Settings holds only fields whose defaults the spec states:** talk mode (hold), talk hotkey (Option+Space), chat panel hotkey (Option+Shift+Space), end-of-speech silence (800 ms), screenshot long edge (1600 px), launch at login (off). Voice/speed and model/provider are added by the steps that define them; `#[serde(default)]` makes an added field load from an older file without a migration.
- **mmproj precision is F16 for every tier.** It is the reference precision, and reading on-screen text is the core use. The screen-qa eval (step 9) can revisit Q8_0.
- **Paths are the caller's.** No crate hardcodes `Hey Dot`; the app computes both folders when it first calls these crates (step 3).

## Review Focus

- App killed after the last byte arrived but before the rename (`.part` already full size): expect the next call to verify and install it with no HTTP request, never a `Range: bytes=<size>-` that answers 416. Task 5 test `complete_partial_file_is_verified_and_installed_without_a_request`.
- Server or CDN ignores `Range` and answers 200 with the whole file: expect the partial file to restart from zero, not have the full body appended. Task 5 test `server_ignoring_range_restarts_the_file_instead_of_appending`.
- A `.part` left from a different upstream revision or a corrupted disk write: expect a hash mismatch that deletes it, then a clean retry succeeds. Task 5 test `stale_partial_file_fails_verification_then_the_retry_succeeds`.
- Settings file hand-edited with a bad value, or written by a newer Hey Dot: expect `load_settings` to return an error naming the problem and leave the file byte-for-byte untouched (the caller decides). Task 1 tests `invalid_value_is_an_error_naming_the_key` and `file_from_a_newer_version_is_rejected_and_left_untouched`.
- A 16 GB Mac reports exactly 16 GiB: expect 4B, while 1 byte less gets 2B. Task 4 tests `apple_silicon_16gb_gets_4b` and `apple_silicon_just_under_16gb_gets_2b_with_a_quality_warning`.

---

### Task 1: dot-settings crate with the settings file

**Files:**
- Modify: `Cargo.toml` (workspace members)
- Create: `crates/dot-settings/Cargo.toml`
- Create: `crates/dot-settings/src/lib.rs`
- Create: `crates/dot-settings/src/settings_file.rs`
- Create: `crates/dot-settings/tests/settings_file.rs`
- Create: `crates/dot-settings/AGENTS.md`
- Modify: `AGENTS.md` (one module line)

**Interfaces:**
- Consumes: nothing.
- Produces: `dot_settings::{Settings, TalkMode, SettingsError, CURRENT_SETTINGS_VERSION, load_settings(&Path) -> Result<Settings, SettingsError>, save_settings(&Path, &Settings) -> Result<(), SettingsError>}`.

- [ ] **Step 1: Add the crate to the workspace with an empty library**

In root `Cargo.toml` change the members line to:

```toml
members = ["app/src-tauri", "crates/*"]
```

Create `crates/dot-settings/Cargo.toml`:

```toml
[package]
name = "dot-settings"
version = "0.1.0"
edition = "2024"
license = "Apache-2.0"
publish = false

[dependencies]
serde = { version = "1.0.229", features = ["derive"] }
thiserror = "2.0.20"
toml = "1.1.6"

[dev-dependencies]
tempfile = "3.27.0"
```

Create `crates/dot-settings/src/lib.rs`:

```rust
//! Hey Dot's user settings file and the API keys that must never be in it.
//! Must not know about the UI, models or providers, or where the settings file lives.
```

- [ ] **Step 2: Write the failing tests**

Create `crates/dot-settings/tests/settings_file.rs`:

```rust
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
```

- [ ] **Step 3: Run the tests to verify they fail**

Run: `cargo test -p dot-settings`
Expected: compile error `unresolved imports` for `dot_settings::CURRENT_SETTINGS_VERSION`, `Settings`, `SettingsError`, `TalkMode`, `load_settings`, `save_settings`.

- [ ] **Step 4: Implement the settings file**

Create `crates/dot-settings/src/settings_file.rs`:

```rust
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
```

Append to `crates/dot-settings/src/lib.rs`:

```rust

mod settings_file;

pub use settings_file::{
    CURRENT_SETTINGS_VERSION, Settings, SettingsError, TalkMode, load_settings, save_settings,
};
```

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test -p dot-settings`
Expected: `test result: ok. 6 passed; 0 failed`.

- [ ] **Step 6: Write the module doc and route to it**

Create `crates/dot-settings/AGENTS.md`:

```markdown
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
```

In root `AGENTS.md`, under `## Modules`, after the `app/` line add:

```markdown
- `crates/dot-settings/`: settings file and API keys. See `crates/dot-settings/AGENTS.md`.
```

- [ ] **Step 7: Format, lint, commit**

```bash
cargo fmt --all
cargo clippy -p dot-settings --all-targets -- -D warnings
cargo test -p dot-settings
git add Cargo.toml Cargo.lock crates/dot-settings AGENTS.md
git commit -m "Add dot-settings crate with the typed settings file"
```
Expected: clippy prints no warnings; tests `6 passed`.

---

### Task 2: API keys in the macOS Keychain

**Files:**
- Modify: `crates/dot-settings/Cargo.toml`
- Create: `crates/dot-settings/src/api_keys.rs`
- Modify: `crates/dot-settings/src/lib.rs`
- Create: `crates/dot-settings/tests/api_keys.rs`
- Modify: `crates/dot-settings/AGENTS.md`

**Interfaces:**
- Consumes: the crate from Task 1.
- Produces: `dot_settings::{use_macos_keychain() -> Result<(), ApiKeyError>, save_api_key(provider_id: &str, api_key: &str) -> Result<(), ApiKeyError>, read_api_key(provider_id: &str) -> Result<Option<String>, ApiKeyError>, ApiKeyError}`.

- [ ] **Step 1: Add the keyring dependencies**

In `crates/dot-settings/Cargo.toml` `[dependencies]` add:

```toml
apple-native-keyring-store = { version = "1", features = ["keychain"] }
keyring-core = "1.0.0"
```

- [ ] **Step 2: Write the failing tests**

Create `crates/dot-settings/tests/api_keys.rs`:

```rust
use std::sync::Once;

use dot_settings::{read_api_key, save_api_key};

// The default credential store is process-global and tests run in parallel threads, so it
// is set once and each test uses its own provider id.
fn use_mock_keychain() {
    static MOCK_KEYCHAIN: Once = Once::new();
    MOCK_KEYCHAIN.call_once(|| {
        keyring_core::set_default_store(keyring_core::mock::Store::new().unwrap());
    });
}

#[test]
fn saved_key_reads_back() {
    use_mock_keychain();

    save_api_key("provider-saved", "sk-first").unwrap();

    assert_eq!(
        read_api_key("provider-saved").unwrap(),
        Some("sk-first".to_string())
    );
}

#[test]
fn saving_again_replaces_the_key() {
    use_mock_keychain();

    save_api_key("provider-replaced", "sk-old").unwrap();
    save_api_key("provider-replaced", "sk-new").unwrap();

    assert_eq!(
        read_api_key("provider-replaced").unwrap(),
        Some("sk-new".to_string())
    );
}

#[test]
fn provider_without_a_key_reads_none() {
    use_mock_keychain();

    assert_eq!(read_api_key("provider-never-saved").unwrap(), None);
}
```

- [ ] **Step 3: Run the tests to verify they fail**

Run: `cargo test -p dot-settings --test api_keys`
Expected: compile error `unresolved imports` for `dot_settings::read_api_key` and `save_api_key`.

- [ ] **Step 4: Implement the keychain access**

Create `crates/dot-settings/src/api_keys.rs`:

```rust
//! Cloud provider API keys, kept in the OS credential store so they never reach the
//! settings file or logs.
//! Must not log, print or persist a key anywhere; `read_api_key`'s return value is the
//! only way a key leaves this file.

const KEYCHAIN_SERVICE: &str = "com.shub3am.heydot";

#[derive(Debug, thiserror::Error)]
#[error("keychain error: {0}")]
pub struct ApiKeyError(#[from] keyring_core::Error);

/// Called once at app start, before any key is read or saved.
pub fn use_macos_keychain() -> Result<(), ApiKeyError> {
    keyring_core::set_default_store(apple_native_keyring_store::keychain::Store::new()?);
    Ok(())
}

pub fn save_api_key(provider_id: &str, api_key: &str) -> Result<(), ApiKeyError> {
    keyring_core::Entry::new(KEYCHAIN_SERVICE, provider_id)?.set_password(api_key)?;
    Ok(())
}

pub fn read_api_key(provider_id: &str) -> Result<Option<String>, ApiKeyError> {
    match keyring_core::Entry::new(KEYCHAIN_SERVICE, provider_id)?.get_password() {
        Ok(api_key) => Ok(Some(api_key)),
        Err(keyring_core::Error::NoEntry) => Ok(None),
        Err(error) => Err(error.into()),
    }
}
```

In `crates/dot-settings/src/lib.rs` add `mod api_keys;` above `mod settings_file;` and add the re-export:

```rust
pub use api_keys::{ApiKeyError, read_api_key, save_api_key, use_macos_keychain};
```

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test -p dot-settings`
Expected: `api_keys` target `3 passed`, `settings_file` target `6 passed`.

- [ ] **Step 6: Update the module doc**

Replace `crates/dot-settings/AGENTS.md` with:

```markdown
# dot-settings

Owns: the user settings file (typed `Settings`, defaults, TOML load and save, format version) and cloud API keys in the macOS Keychain.

Must not know about: the UI, models, providers, or where the file lives. The caller passes the path; the app uses `~/Library/Application Support/Hey Dot/settings.toml`. Provider ids are opaque strings chosen by the caller.

Entry points: `load_settings(path)`, `save_settings(path, &settings)`, `Settings::default()`; `use_macos_keychain()` once at app start, then `save_api_key` and `read_api_key`.

Invariants and gotchas:
- A missing file loads defaults and writes nothing. Saving is the caller's decision.
- A file that fails to parse, or has a `version` above `CURRENT_SETTINGS_VERSION`, is an error and is never rewritten by this crate. A caller that saves defaults after that error destroys the user's file; show the error instead.
- Adding a field needs no version bump: every field has a default. Bump the version only when a field changes meaning or type, and add the migration in `load_settings` in the same commit.
- Unknown keys are ignored so an older build can read a newer file; saving from the older build drops those keys.
- Saves go through `settings.toml.tmp` and a rename, so a crash never leaves a half-written file.
- Hotkey strings use the global-hotkey format (`Alt+Space`); validation happens where the shortcut is registered.
- API keys never go in `Settings`, the TOML, or any log line. Keychain service is `com.shub3am.heydot`, account is the provider id.
- The credential store is process-global: without `use_macos_keychain()` every key call fails. Tests set `keyring_core::mock::Store` once instead; the real Keychain path is not exercised in CI.

Called by: `app/src-tauri` (from Phase 1 step 3 on).
```

- [ ] **Step 7: Format, lint, commit**

```bash
cargo fmt --all
cargo clippy -p dot-settings --all-targets -- -D warnings
cargo test -p dot-settings
git add crates/dot-settings Cargo.lock
git commit -m "Store API keys in the macOS Keychain via keyring-core"
```
Expected: no clippy warnings; `9 passed` across both targets.

---

### Task 3: dot-models crate with the pinned catalog

**Files:**
- Create: `crates/dot-models/Cargo.toml`
- Create: `crates/dot-models/src/lib.rs`
- Create: `crates/dot-models/src/catalog.rs`
- Create: `crates/dot-models/tests/catalog.rs`
- Create: `crates/dot-models/AGENTS.md`
- Modify: `AGENTS.md` (one module line)

**Interfaces:**
- Consumes: nothing.
- Produces: `dot_models::{Model { id, display_name, min_ram_gb: u64, files: &'static [ModelFile] }, ModelFile { name, url, sha256, bytes: u64 }, CATALOG: [&Model; 5], QWEN3_VL_2B, QWEN3_VL_4B, QWEN3_VL_8B, PARAKEET_TDT_V3, SILERO_VAD_V6}`. All string fields are `&'static str`. `Model` and `ModelFile` derive `Debug, PartialEq, Eq`.

- [ ] **Step 1: Create the crate**

Create `crates/dot-models/Cargo.toml`:

```toml
[package]
name = "dot-models"
version = "0.1.0"
edition = "2024"
license = "Apache-2.0"
publish = false

[dependencies]
```

Create `crates/dot-models/src/lib.rs`:

```rust
//! Which models Hey Dot can use, which one suits this Mac, and getting their files on disk.
//! Must not run models, read settings, know about the UI, or decide where models are stored.
```

- [ ] **Step 2: Write the failing tests**

Create `crates/dot-models/tests/catalog.rs`:

```rust
use std::collections::HashSet;

use dot_models::CATALOG;

fn is_lowercase_hex(text: &str) -> bool {
    text.chars().all(|character| matches!(character, '0'..='9' | 'a'..='f'))
}

#[test]
fn every_file_url_is_https_pinned_to_a_commit_and_ends_in_its_name() {
    for model in CATALOG {
        for file in model.files {
            assert!(file.url.starts_with("https://"), "{}", file.url);
            assert!(file.url.ends_with(&format!("/{}", file.name)), "{}", file.url);
            assert!(
                file.url
                    .split('/')
                    .any(|segment| segment.len() == 40 && is_lowercase_hex(segment)),
                "{} is not pinned to a commit",
                file.url
            );
        }
    }
}

#[test]
fn every_file_has_a_sha256_and_a_size() {
    for model in CATALOG {
        for file in model.files {
            assert!(
                file.sha256.len() == 64 && is_lowercase_hex(file.sha256),
                "{}",
                file.name
            );
            assert!(file.bytes > 0, "{}", file.name);
        }
    }
}

#[test]
fn model_ids_are_unique() {
    let ids: HashSet<_> = CATALOG.iter().map(|model| model.id).collect();
    assert_eq!(ids.len(), CATALOG.len());
}

#[test]
fn file_names_are_unique_within_each_model() {
    for model in CATALOG {
        let names: HashSet<_> = model.files.iter().map(|file| file.name).collect();
        assert_eq!(names.len(), model.files.len(), "{}", model.id);
    }
}
```

- [ ] **Step 3: Run the tests to verify they fail**

Run: `cargo test -p dot-models`
Expected: compile error `unresolved import dot_models::CATALOG`.

- [ ] **Step 4: Implement the catalog**

Create `crates/dot-models/src/catalog.rs` (values from the HuggingFace API and GitHub on 2026-09-23; each URL is pinned to an upstream commit so its sha256 cannot drift):

```rust
//! The compiled-in list of models Hey Dot can download, each file pinned to an upstream
//! commit with its sha256 and size.
//! Must not fetch anything or decide which model a machine should use.

#[derive(Debug, PartialEq, Eq)]
pub struct ModelFile {
    pub name: &'static str,
    pub url: &'static str,
    pub sha256: &'static str,
    pub bytes: u64,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Model {
    pub id: &'static str,
    pub display_name: &'static str,
    /// Smallest RAM, in GiB, the model is meant to run on. 0 for models too small to matter.
    pub min_ram_gb: u64,
    pub files: &'static [ModelFile],
}

pub static QWEN3_VL_2B: Model = Model {
    id: "qwen3-vl-2b-instruct-q4km",
    display_name: "Qwen3-VL 2B",
    min_ram_gb: 8,
    files: &[
        ModelFile {
            name: "Qwen3VL-2B-Instruct-Q4_K_M.gguf",
            url: "https://huggingface.co/Qwen/Qwen3-VL-2B-Instruct-GGUF/resolve/52d6c8ffea26cc873ac5ad116f8631268d7eb503/Qwen3VL-2B-Instruct-Q4_K_M.gguf",
            sha256: "089d75c52f4b7ffc56ba998ffc50aae89fcafc755f9e7208aacca281dca6c2ae",
            bytes: 1_107_409_952,
        },
        ModelFile {
            name: "mmproj-Qwen3VL-2B-Instruct-F16.gguf",
            url: "https://huggingface.co/Qwen/Qwen3-VL-2B-Instruct-GGUF/resolve/52d6c8ffea26cc873ac5ad116f8631268d7eb503/mmproj-Qwen3VL-2B-Instruct-F16.gguf",
            sha256: "c3d5afbef5287953acd57b4043d2269456e5761a4eaccb3b71b062996970aea5",
            bytes: 819_394_848,
        },
    ],
};

pub static QWEN3_VL_4B: Model = Model {
    id: "qwen3-vl-4b-instruct-q4km",
    display_name: "Qwen3-VL 4B",
    min_ram_gb: 16,
    files: &[
        ModelFile {
            name: "Qwen3VL-4B-Instruct-Q4_K_M.gguf",
            url: "https://huggingface.co/Qwen/Qwen3-VL-4B-Instruct-GGUF/resolve/1cd86afb9a95c410a6038ab3b40d8b578c892266/Qwen3VL-4B-Instruct-Q4_K_M.gguf",
            sha256: "66358cb18bb6b3b1b6675aa412c7a88ef01d228f481184d13668e5201c730a0a",
            bytes: 2_497_281_664,
        },
        ModelFile {
            name: "mmproj-Qwen3VL-4B-Instruct-F16.gguf",
            url: "https://huggingface.co/Qwen/Qwen3-VL-4B-Instruct-GGUF/resolve/1cd86afb9a95c410a6038ab3b40d8b578c892266/mmproj-Qwen3VL-4B-Instruct-F16.gguf",
            sha256: "256f3a43bd4205ffef48d6b92715e1e70b5b0e9aef06522584967513a9985331",
            bytes: 836_180_256,
        },
    ],
};

pub static QWEN3_VL_8B: Model = Model {
    id: "qwen3-vl-8b-instruct-q4km",
    display_name: "Qwen3-VL 8B",
    min_ram_gb: 32,
    files: &[
        ModelFile {
            name: "Qwen3VL-8B-Instruct-Q4_K_M.gguf",
            url: "https://huggingface.co/Qwen/Qwen3-VL-8B-Instruct-GGUF/resolve/f982a07559d4a2f6c8744d840bf6fccab30eea96/Qwen3VL-8B-Instruct-Q4_K_M.gguf",
            sha256: "67d1659bfe71b89d50b45a4ad1a9e5b997e5bb16ce5da66a6a6167abd569e9e2",
            bytes: 5_027_784_800,
        },
        ModelFile {
            name: "mmproj-Qwen3VL-8B-Instruct-F16.gguf",
            url: "https://huggingface.co/Qwen/Qwen3-VL-8B-Instruct-GGUF/resolve/f982a07559d4a2f6c8744d840bf6fccab30eea96/mmproj-Qwen3VL-8B-Instruct-F16.gguf",
            sha256: "ca524100ebf825c9a870db1c580d03879e0da0ab2541697e2458e64891cf9d38",
            bytes: 1_159_029_824,
        },
    ],
};

pub static PARAKEET_TDT_V3: Model = Model {
    id: "parakeet-tdt-0.6b-v3-int8",
    display_name: "Parakeet TDT 0.6B v3",
    min_ram_gb: 0,
    files: &[
        ModelFile {
            name: "encoder.int8.onnx",
            url: "https://huggingface.co/csukuangfj/sherpa-onnx-nemo-parakeet-tdt-0.6b-v3-int8/resolve/2bda32ec70b097a55adaa07d9a7173915b43cc78/encoder.int8.onnx",
            sha256: "acfc2b4456377e15d04f0243af540b7fe7c992f8d898d751cf134c3a55fd2247",
            bytes: 652_184_281,
        },
        ModelFile {
            name: "decoder.int8.onnx",
            url: "https://huggingface.co/csukuangfj/sherpa-onnx-nemo-parakeet-tdt-0.6b-v3-int8/resolve/2bda32ec70b097a55adaa07d9a7173915b43cc78/decoder.int8.onnx",
            sha256: "179e50c43d1a9de79c8a24149a2f9bac6eb5981823f2a2ed88d655b24248db4e",
            bytes: 11_845_275,
        },
        ModelFile {
            name: "joiner.int8.onnx",
            url: "https://huggingface.co/csukuangfj/sherpa-onnx-nemo-parakeet-tdt-0.6b-v3-int8/resolve/2bda32ec70b097a55adaa07d9a7173915b43cc78/joiner.int8.onnx",
            sha256: "3164c13fc2821009440d20fcb5fdc78bff28b4db2f8d0f0b329101719c0948b3",
            bytes: 6_355_277,
        },
        ModelFile {
            name: "tokens.txt",
            url: "https://huggingface.co/csukuangfj/sherpa-onnx-nemo-parakeet-tdt-0.6b-v3-int8/resolve/2bda32ec70b097a55adaa07d9a7173915b43cc78/tokens.txt",
            sha256: "d58544679ea4bc6ac563d1f545eb7d474bd6cfa467f0a6e2c1dc1c7d37e3c35d",
            bytes: 93_939,
        },
    ],
};

pub static SILERO_VAD_V6: Model = Model {
    id: "silero-vad-v6",
    display_name: "Silero VAD v6",
    min_ram_gb: 0,
    files: &[ModelFile {
        name: "silero_vad.onnx",
        url: "https://raw.githubusercontent.com/snakers4/silero-vad/fba061dc5559f696e62171e9a0741782b0fdc23c/src/silero_vad/data/silero_vad.onnx",
        sha256: "597d30b3ec076608d059477bb14cfeffdf951bf5cae370d38f65d33bbfe82004",
        bytes: 2_327_524,
    }],
};

pub static CATALOG: [&Model; 5] = [
    &QWEN3_VL_2B,
    &QWEN3_VL_4B,
    &QWEN3_VL_8B,
    &PARAKEET_TDT_V3,
    &SILERO_VAD_V6,
];
```

Append to `crates/dot-models/src/lib.rs`:

```rust

mod catalog;

pub use catalog::{
    CATALOG, Model, ModelFile, PARAKEET_TDT_V3, QWEN3_VL_2B, QWEN3_VL_4B, QWEN3_VL_8B,
    SILERO_VAD_V6,
};
```

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test -p dot-models`
Expected: `test result: ok. 4 passed; 0 failed`.

- [ ] **Step 6: Write the module doc and route to it**

Create `crates/dot-models/AGENTS.md`:

```markdown
# dot-models

Owns: the compiled-in catalog of downloadable models (URLs pinned to upstream commits, sha256, sizes).

Must not know about: settings, the UI, Tauri, llama-server or any inference, or where models are stored.

Entry points: `CATALOG` and the per-model statics (`QWEN3_VL_4B`, `PARAKEET_TDT_V3`, ...).

Invariants and gotchas:
- Every URL is pinned to a commit. Changing a file (new commit, other quant) means updating its sha256 and bytes in the same edit.
- Kokoro is not in the catalog yet: its file set depends on the G2P decision in the dot-tts step.
- Qwen3-VL GGUF compatibility with the pinned llama-server is proven in the dot-runtime step, not here.
- Licenses: Qwen3-VL Apache-2.0, Parakeet weights CC-BY-4.0 (attribution needed in the app's about screen), Silero VAD MIT.

Called by: `app/src-tauri` (from Phase 1 step 3 on).
```

In root `AGENTS.md`, under `## Modules`, after the `crates/dot-settings/` line add:

```markdown
- `crates/dot-models/`: model catalog, hardware recommendation, verified downloads. See `crates/dot-models/AGENTS.md`.
```

- [ ] **Step 7: Format, lint, commit**

```bash
cargo fmt --all
cargo clippy -p dot-models --all-targets -- -D warnings
cargo test -p dot-models
git add crates/dot-models Cargo.lock AGENTS.md
git commit -m "Add dot-models crate with the pinned model catalog"
```
Expected: no clippy warnings; `4 passed`.

---

### Task 4: Hardware detection and model recommendation

**Files:**
- Modify: `crates/dot-models/Cargo.toml`
- Create: `crates/dot-models/src/hardware.rs`
- Create: `crates/dot-models/src/recommend.rs`
- Modify: `crates/dot-models/src/lib.rs`
- Create: `crates/dot-models/tests/recommend.rs`
- Modify: `crates/dot-models/AGENTS.md`

**Interfaces:**
- Consumes: `QWEN3_VL_2B`, `QWEN3_VL_4B`, `QWEN3_VL_8B`, `Model` from Task 3.
- Produces: `dot_models::{Chip { AppleSilicon, Intel }, Hardware { chip: Chip, ram_bytes: u64 }, detect_hardware() -> Hardware, ModelChoice { Local(&'static Model), Cloud }, Recommendation { recommended: ModelChoice, also_offered: Vec<ModelChoice>, warn_small_model: bool }, recommend(&Hardware) -> Recommendation}`.

- [ ] **Step 1: Add sysinfo**

In `crates/dot-models/Cargo.toml` `[dependencies]` add:

```toml
sysinfo = { version = "0.39.6", default-features = false, features = ["system"] }
```

- [ ] **Step 2: Write the failing tests**

Create `crates/dot-models/tests/recommend.rs`:

```rust
use dot_models::{
    Chip, Hardware, ModelChoice, QWEN3_VL_2B, QWEN3_VL_4B, QWEN3_VL_8B, Recommendation,
    detect_hardware, recommend,
};

const GIB: u64 = 1024 * 1024 * 1024;

fn apple_silicon_with_ram_bytes(ram_bytes: u64) -> Hardware {
    Hardware {
        chip: Chip::AppleSilicon,
        ram_bytes,
    }
}

#[test]
fn apple_silicon_8gb_gets_2b_with_a_quality_warning() {
    assert_eq!(
        recommend(&apple_silicon_with_ram_bytes(8 * GIB)),
        Recommendation {
            recommended: ModelChoice::Local(&QWEN3_VL_2B),
            also_offered: vec![],
            warn_small_model: true,
        }
    );
}

#[test]
fn apple_silicon_just_under_16gb_gets_2b_with_a_quality_warning() {
    assert_eq!(
        recommend(&apple_silicon_with_ram_bytes(16 * GIB - 1)).recommended,
        ModelChoice::Local(&QWEN3_VL_2B)
    );
}

#[test]
fn apple_silicon_16gb_gets_4b() {
    assert_eq!(
        recommend(&apple_silicon_with_ram_bytes(16 * GIB)),
        Recommendation {
            recommended: ModelChoice::Local(&QWEN3_VL_4B),
            also_offered: vec![],
            warn_small_model: false,
        }
    );
}

#[test]
fn apple_silicon_32gb_gets_4b_and_is_offered_8b() {
    assert_eq!(
        recommend(&apple_silicon_with_ram_bytes(32 * GIB)),
        Recommendation {
            recommended: ModelChoice::Local(&QWEN3_VL_4B),
            also_offered: vec![ModelChoice::Local(&QWEN3_VL_8B)],
            warn_small_model: false,
        }
    );
}

#[test]
fn intel_is_recommended_cloud_with_local_2b_allowed() {
    let intel = Hardware {
        chip: Chip::Intel,
        ram_bytes: 16 * GIB,
    };

    assert_eq!(
        recommend(&intel),
        Recommendation {
            recommended: ModelChoice::Cloud,
            also_offered: vec![ModelChoice::Local(&QWEN3_VL_2B)],
            warn_small_model: false,
        }
    );
}

#[test]
fn detected_hardware_reports_this_machine() {
    let hardware = detect_hardware();

    assert!(hardware.ram_bytes > 0);
    if cfg!(target_arch = "aarch64") {
        assert_eq!(hardware.chip, Chip::AppleSilicon);
    }
}
```

- [ ] **Step 3: Run the tests to verify they fail**

Run: `cargo test -p dot-models --test recommend`
Expected: compile error `unresolved imports` for `Chip`, `Hardware`, `ModelChoice`, `Recommendation`, `detect_hardware`, `recommend`.

- [ ] **Step 4: Implement hardware detection and the recommendation**

Create `crates/dot-models/src/hardware.rs`:

```rust
//! Reads the facts about this Mac that decide which model it can run.
//! Must not decide which model to use; that is `recommend`.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Chip {
    AppleSilicon,
    Intel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Hardware {
    pub chip: Chip,
    pub ram_bytes: u64,
}

pub fn detect_hardware() -> Hardware {
    let mut system = sysinfo::System::new();
    system.refresh_memory();
    // The architecture this binary was compiled for. The universal build runs its arm64
    // slice on Apple Silicon, so this is only wrong when the user forces Rosetta.
    let chip = if std::env::consts::ARCH == "aarch64" {
        Chip::AppleSilicon
    } else {
        Chip::Intel
    };
    Hardware {
        chip,
        ram_bytes: system.total_memory(),
    }
}
```

Create `crates/dot-models/src/recommend.rs`:

```rust
//! Picks the chat model onboarding suggests for a machine.
//! Must not detect hardware or download anything. Cloud is always available in onboarding,
//! so it only appears here when it is the recommendation.

use crate::catalog::{Model, QWEN3_VL_2B, QWEN3_VL_4B, QWEN3_VL_8B};
use crate::hardware::{Chip, Hardware};

const BYTES_PER_GIB: u64 = 1024 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModelChoice {
    Local(&'static Model),
    Cloud,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Recommendation {
    pub recommended: ModelChoice,
    pub also_offered: Vec<ModelChoice>,
    pub warn_small_model: bool,
}

pub fn recommend(hardware: &Hardware) -> Recommendation {
    let ram_gb = hardware.ram_bytes / BYTES_PER_GIB;
    match hardware.chip {
        Chip::Intel => Recommendation {
            recommended: ModelChoice::Cloud,
            also_offered: vec![ModelChoice::Local(&QWEN3_VL_2B)],
            warn_small_model: false,
        },
        Chip::AppleSilicon if ram_gb >= QWEN3_VL_8B.min_ram_gb => Recommendation {
            recommended: ModelChoice::Local(&QWEN3_VL_4B),
            also_offered: vec![ModelChoice::Local(&QWEN3_VL_8B)],
            warn_small_model: false,
        },
        Chip::AppleSilicon if ram_gb >= QWEN3_VL_4B.min_ram_gb => Recommendation {
            recommended: ModelChoice::Local(&QWEN3_VL_4B),
            also_offered: vec![],
            warn_small_model: false,
        },
        Chip::AppleSilicon => Recommendation {
            recommended: ModelChoice::Local(&QWEN3_VL_2B),
            also_offered: vec![],
            warn_small_model: true,
        },
    }
}
```

In `crates/dot-models/src/lib.rs` add `mod hardware;` and `mod recommend;` beside `mod catalog;`, and the re-exports:

```rust
pub use hardware::{Chip, Hardware, detect_hardware};
pub use recommend::{ModelChoice, Recommendation, recommend};
```

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test -p dot-models`
Expected: `catalog` target `4 passed`, `recommend` target `6 passed`.

- [ ] **Step 6: Update the module doc**

Replace `crates/dot-models/AGENTS.md` with:

```markdown
# dot-models

Owns: the compiled-in catalog of downloadable models (URLs pinned to upstream commits, sha256, sizes) and the hardware-based recommendation of which chat model to use.

Must not know about: settings, the UI, Tauri, llama-server or any inference, or where models are stored.

Entry points: `CATALOG` and the per-model statics (`QWEN3_VL_4B`, `PARAKEET_TDT_V3`, ...); `detect_hardware()` then `recommend(&hardware)`.

Invariants and gotchas:
- Every URL is pinned to a commit. Changing a file (new commit, other quant) means updating its sha256 and bytes in the same edit.
- Kokoro is not in the catalog yet: its file set depends on the G2P decision in the dot-tts step.
- Qwen3-VL GGUF compatibility with the pinned llama-server is proven in the dot-runtime step, not here.
- Licenses: Qwen3-VL Apache-2.0, Parakeet weights CC-BY-4.0 (attribution needed in the app's about screen), Silero VAD MIT.
- `recommend` thresholds are the chat models' `min_ram_gb` (4B: 16, 8B: 32), compared in whole GiB rounded down. Cloud is only returned for Intel; onboarding offers cloud to everyone anyway.
- The chip is the architecture the binary was compiled for. An Apple Silicon Mac running the app under Rosetta is seen as Intel.

Called by: `app/src-tauri` (from Phase 1 step 3 on).
```

- [ ] **Step 7: Format, lint, commit**

```bash
cargo fmt --all
cargo clippy -p dot-models --all-targets -- -D warnings
cargo test -p dot-models
git add crates/dot-models Cargo.lock
git commit -m "Recommend a chat model tier from the Mac's chip and RAM"
```
Expected: no clippy warnings; `10 passed` across both targets.

---

### Task 5: Resumable, sha256-verified model downloads

**Files:**
- Modify: `crates/dot-models/Cargo.toml`
- Create: `crates/dot-models/src/download.rs`
- Modify: `crates/dot-models/src/lib.rs`
- Create: `crates/dot-models/tests/download.rs`
- Modify: `crates/dot-models/tests/catalog.rs` (network check)
- Modify: `crates/dot-models/AGENTS.md`
- Modify: `AGENTS.md` (Test section line)

**Interfaces:**
- Consumes: `Model`, `ModelFile`, `CATALOG`, `SILERO_VAD_V6` from Task 3.
- Produces: `dot_models::{download_model(client: &reqwest::Client, model: &Model, models_dir: &Path, on_progress: impl FnMut(DownloadProgress)) -> Result<PathBuf, DownloadError>` (async; returns `<models_dir>/<model.id>`), `DownloadProgress { downloaded_bytes: u64, total_bytes: u64 }` (model-wide), `DownloadError { Request(reqwest::Error), UnexpectedStatus { url: String, status: u16 }, Io(std::io::Error), HashMismatch { file_name: String, expected: String, actual: String } }}`.

- [ ] **Step 1: Add the download dependencies**

In `crates/dot-models/Cargo.toml` `[dependencies]` add:

```toml
futures-util = "0.3.34"
hex = "0.4.3"
reqwest = { version = "0.13.5", features = ["stream"] }
sha2 = "0.11.0"
thiserror = "2.0.20"
tokio = { version = "1.53.1", features = ["fs", "io-util", "rt"] }
```

and add a dev-dependencies section:

```toml
[dev-dependencies]
axum = "0.8.9"
tempfile = "3.27.0"
tokio = { version = "1.53.1", features = ["macros", "net", "rt-multi-thread"] }
tower-http = { version = "0.7.1", features = ["fs"] }
```

- [ ] **Step 2: Write the failing tests**

Create `crates/dot-models/tests/download.rs`:

```rust
use std::path::Path;
use std::sync::{Arc, Mutex};

use axum::Router;
use axum::extract::Request;
use axum::http::header;
use axum::middleware::{self, Next};
use axum::routing::get;
use dot_models::{
    DownloadError, DownloadProgress, Model, ModelFile, SILERO_VAD_V6, download_model,
};
use sha2::{Digest, Sha256};
use tower_http::services::ServeDir;

struct FileServer {
    base_url: String,
    range_headers: Arc<Mutex<Vec<Option<String>>>>,
}

impl FileServer {
    /// The Range header of every request so far, in order; `None` means no Range header.
    fn range_headers_seen(&self) -> Vec<Option<String>> {
        self.range_headers.lock().unwrap().clone()
    }
}

/// Serves `served_dir` with Range support, plus `/ignores-range/<name>` which always answers
/// 200 with `ignores_range_body`, like a server without Range support.
async fn start_file_server(served_dir: &Path, ignores_range_body: Vec<u8>) -> FileServer {
    let range_headers = Arc::new(Mutex::new(Vec::new()));
    let recorded_range_headers = range_headers.clone();
    let app = Router::new()
        .route(
            "/ignores-range/{name}",
            get(move || {
                let body = ignores_range_body.clone();
                async move { body }
            }),
        )
        .fallback_service(ServeDir::new(served_dir))
        .layer(middleware::from_fn(move |request: Request, next: Next| {
            let recorded_range_headers = recorded_range_headers.clone();
            async move {
                let range = request
                    .headers()
                    .get(header::RANGE)
                    .map(|value| value.to_str().unwrap().to_string());
                recorded_range_headers.lock().unwrap().push(range);
                next.run(request).await
            }
        }));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base_url = format!("http://{}", listener.local_addr().unwrap());
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    FileServer {
        base_url,
        range_headers,
    }
}

/// Bytes that differ at every offset near each other, so a wrong resume offset corrupts the hash.
fn patterned_bytes(length: usize) -> Vec<u8> {
    (0..length).map(|index| (index % 251) as u8).collect()
}

fn sha256_hex(content: &[u8]) -> String {
    hex::encode(Sha256::digest(content))
}

// Catalog entries are `'static`; tests leak their few strings rather than widen the types.
fn leak(text: String) -> &'static str {
    Box::leak(text.into_boxed_str())
}

fn test_file(url: String, name: &'static str, content: &[u8]) -> ModelFile {
    ModelFile {
        name,
        url: leak(url),
        sha256: leak(sha256_hex(content)),
        bytes: content.len() as u64,
    }
}

fn test_model(files: Vec<ModelFile>) -> Model {
    Model {
        id: "test-model",
        display_name: "Test model",
        min_ram_gb: 0,
        files: Box::leak(files.into_boxed_slice()),
    }
}

#[tokio::test]
async fn fresh_download_writes_verified_files_and_reports_progress_to_the_total() {
    let served_dir = tempfile::tempdir().unwrap();
    let first = patterned_bytes(300_000);
    let second = patterned_bytes(70_000);
    std::fs::write(served_dir.path().join("first.bin"), &first).unwrap();
    std::fs::write(served_dir.path().join("second.bin"), &second).unwrap();
    let server = start_file_server(served_dir.path(), vec![]).await;
    let model = test_model(vec![
        test_file(format!("{}/first.bin", server.base_url), "first.bin", &first),
        test_file(format!("{}/second.bin", server.base_url), "second.bin", &second),
    ]);
    let models_dir = tempfile::tempdir().unwrap();
    let mut progress_updates = Vec::new();

    let model_dir = download_model(&reqwest::Client::new(), &model, models_dir.path(), |update| {
        progress_updates.push(update)
    })
    .await
    .unwrap();

    assert_eq!(model_dir, models_dir.path().join("test-model"));
    assert_eq!(std::fs::read(model_dir.join("first.bin")).unwrap(), first);
    assert_eq!(std::fs::read(model_dir.join("second.bin")).unwrap(), second);
    assert!(!model_dir.join("first.bin.part").exists());
    assert!(!model_dir.join("second.bin.part").exists());
    assert!(
        progress_updates
            .windows(2)
            .all(|pair| pair[0].downloaded_bytes <= pair[1].downloaded_bytes)
    );
    assert_eq!(
        progress_updates.last(),
        Some(&DownloadProgress {
            downloaded_bytes: 370_000,
            total_bytes: 370_000
        })
    );
    assert_eq!(server.range_headers_seen(), vec![None, None]);
}

#[tokio::test]
async fn interrupted_download_resumes_with_a_range_request() {
    let served_dir = tempfile::tempdir().unwrap();
    let content = patterned_bytes(200_000);
    std::fs::write(served_dir.path().join("model.bin"), &content).unwrap();
    let server = start_file_server(served_dir.path(), vec![]).await;
    let model = test_model(vec![test_file(
        format!("{}/model.bin", server.base_url),
        "model.bin",
        &content,
    )]);
    let models_dir = tempfile::tempdir().unwrap();
    let model_dir = models_dir.path().join("test-model");
    std::fs::create_dir_all(&model_dir).unwrap();
    std::fs::write(model_dir.join("model.bin.part"), &content[..120_000]).unwrap();

    download_model(&reqwest::Client::new(), &model, models_dir.path(), |_| {})
        .await
        .unwrap();

    assert_eq!(std::fs::read(model_dir.join("model.bin")).unwrap(), content);
    assert_eq!(
        server.range_headers_seen(),
        vec![Some("bytes=120000-".to_string())]
    );
}

#[tokio::test]
async fn server_ignoring_range_restarts_the_file_instead_of_appending() {
    let served_dir = tempfile::tempdir().unwrap();
    let content = patterned_bytes(50_000);
    let server = start_file_server(served_dir.path(), content.clone()).await;
    let model = test_model(vec![test_file(
        format!("{}/ignores-range/model.bin", server.base_url),
        "model.bin",
        &content,
    )]);
    let models_dir = tempfile::tempdir().unwrap();
    let model_dir = models_dir.path().join("test-model");
    std::fs::create_dir_all(&model_dir).unwrap();
    std::fs::write(model_dir.join("model.bin.part"), vec![0xAA; 10_000]).unwrap();

    download_model(&reqwest::Client::new(), &model, models_dir.path(), |_| {})
        .await
        .unwrap();

    assert_eq!(std::fs::read(model_dir.join("model.bin")).unwrap(), content);
    assert_eq!(
        server.range_headers_seen(),
        vec![Some("bytes=10000-".to_string())]
    );
}

#[tokio::test]
async fn hash_mismatch_deletes_the_partial_file_and_reports_it() {
    let served_dir = tempfile::tempdir().unwrap();
    let content = patterned_bytes(40_000);
    std::fs::write(served_dir.path().join("model.bin"), &content).unwrap();
    let server = start_file_server(served_dir.path(), vec![]).await;
    let model = test_model(vec![ModelFile {
        sha256: leak(sha256_hex(b"different bytes")),
        ..test_file(
            format!("{}/model.bin", server.base_url),
            "model.bin",
            &content,
        )
    }]);
    let models_dir = tempfile::tempdir().unwrap();
    let model_dir = models_dir.path().join("test-model");

    let error = download_model(&reqwest::Client::new(), &model, models_dir.path(), |_| {})
        .await
        .unwrap_err();

    assert!(matches!(
        error,
        DownloadError::HashMismatch { ref file_name, ref actual, .. }
            if file_name == "model.bin" && *actual == sha256_hex(&content)
    ));
    assert!(!model_dir.join("model.bin.part").exists());
    assert!(!model_dir.join("model.bin").exists());
}

#[tokio::test]
async fn stale_partial_file_fails_verification_then_the_retry_succeeds() {
    let served_dir = tempfile::tempdir().unwrap();
    let content = patterned_bytes(60_000);
    std::fs::write(served_dir.path().join("model.bin"), &content).unwrap();
    let server = start_file_server(served_dir.path(), vec![]).await;
    let model = test_model(vec![test_file(
        format!("{}/model.bin", server.base_url),
        "model.bin",
        &content,
    )]);
    let models_dir = tempfile::tempdir().unwrap();
    let model_dir = models_dir.path().join("test-model");
    std::fs::create_dir_all(&model_dir).unwrap();
    std::fs::write(model_dir.join("model.bin.part"), vec![0x55; 20_000]).unwrap();
    let client = reqwest::Client::new();

    let first_attempt = download_model(&client, &model, models_dir.path(), |_| {}).await;
    let second_attempt = download_model(&client, &model, models_dir.path(), |_| {}).await;

    assert!(matches!(first_attempt, Err(DownloadError::HashMismatch { .. })));
    assert!(second_attempt.is_ok());
    assert_eq!(std::fs::read(model_dir.join("model.bin")).unwrap(), content);
    assert_eq!(
        server.range_headers_seen(),
        vec![Some("bytes=20000-".to_string()), None]
    );
}

#[tokio::test]
async fn complete_partial_file_is_verified_and_installed_without_a_request() {
    let served_dir = tempfile::tempdir().unwrap();
    let content = patterned_bytes(30_000);
    let server = start_file_server(served_dir.path(), vec![]).await;
    let model = test_model(vec![test_file(
        format!("{}/model.bin", server.base_url),
        "model.bin",
        &content,
    )]);
    let models_dir = tempfile::tempdir().unwrap();
    let model_dir = models_dir.path().join("test-model");
    std::fs::create_dir_all(&model_dir).unwrap();
    std::fs::write(model_dir.join("model.bin.part"), &content).unwrap();

    download_model(&reqwest::Client::new(), &model, models_dir.path(), |_| {})
        .await
        .unwrap();

    assert_eq!(std::fs::read(model_dir.join("model.bin")).unwrap(), content);
    assert!(!model_dir.join("model.bin.part").exists());
    assert_eq!(server.range_headers_seen(), Vec::<Option<String>>::new());
}

#[tokio::test]
async fn installed_file_is_not_downloaded_again() {
    let served_dir = tempfile::tempdir().unwrap();
    let content = patterned_bytes(30_000);
    let server = start_file_server(served_dir.path(), vec![]).await;
    let model = test_model(vec![test_file(
        format!("{}/model.bin", server.base_url),
        "model.bin",
        &content,
    )]);
    let models_dir = tempfile::tempdir().unwrap();
    let model_dir = models_dir.path().join("test-model");
    std::fs::create_dir_all(&model_dir).unwrap();
    std::fs::write(model_dir.join("model.bin"), &content).unwrap();
    let mut progress_updates = Vec::new();

    download_model(&reqwest::Client::new(), &model, models_dir.path(), |update| {
        progress_updates.push(update)
    })
    .await
    .unwrap();

    assert_eq!(server.range_headers_seen(), Vec::<Option<String>>::new());
    assert_eq!(
        progress_updates.last(),
        Some(&DownloadProgress {
            downloaded_bytes: 30_000,
            total_bytes: 30_000
        })
    );
}

#[tokio::test]
async fn missing_file_on_the_server_is_a_status_error() {
    let served_dir = tempfile::tempdir().unwrap();
    let server = start_file_server(served_dir.path(), vec![]).await;
    let model = test_model(vec![test_file(
        format!("{}/missing.bin", server.base_url),
        "missing.bin",
        b"never served",
    )]);
    let models_dir = tempfile::tempdir().unwrap();
    let model_dir = models_dir.path().join("test-model");

    let error = download_model(&reqwest::Client::new(), &model, models_dir.path(), |_| {})
        .await
        .unwrap_err();

    assert!(matches!(
        error,
        DownloadError::UnexpectedStatus { status: 404, .. }
    ));
    assert!(!model_dir.join("missing.bin.part").exists());
    assert!(!model_dir.join("missing.bin").exists());
}

#[tokio::test]
#[ignore = "network: downloads Silero VAD (2.3 MB) from GitHub and verifies its sha256"]
async fn silero_vad_downloads_and_verifies() {
    let models_dir = tempfile::tempdir().unwrap();

    let model_dir = download_model(
        &reqwest::Client::new(),
        &SILERO_VAD_V6,
        models_dir.path(),
        |_| {},
    )
    .await
    .unwrap();

    assert!(model_dir.join("silero_vad.onnx").exists());
}
```

Append to `crates/dot-models/tests/catalog.rs`:

```rust

#[tokio::test]
#[ignore = "network: asks every catalog URL for one byte and checks the file's total size"]
async fn every_catalog_url_serves_the_catalog_size() {
    let client = reqwest::Client::new();
    for model in CATALOG {
        for file in model.files {
            let response = client
                .get(file.url)
                .header(reqwest::header::RANGE, "bytes=0-0")
                .send()
                .await
                .unwrap();
            assert_eq!(response.status(), reqwest::StatusCode::PARTIAL_CONTENT, "{}", file.url);
            assert_eq!(
                response.headers()[reqwest::header::CONTENT_RANGE],
                format!("bytes 0-0/{}", file.bytes).as_str(),
                "{}",
                file.url
            );
        }
    }
}
```

- [ ] **Step 3: Run the tests to verify they fail**

Run: `cargo test -p dot-models --test download`
Expected: compile error `unresolved imports` for `DownloadError`, `DownloadProgress`, `download_model`.

- [ ] **Step 4: Implement the download**

Create `crates/dot-models/src/download.rs`:

```rust
//! Gets a catalog model's files onto disk, resuming partial files and only ever giving a
//! file its final name after its sha256 matched.
//! Must not choose which model to download or where the models folder is.

use std::io::Read;
use std::path::{Path, PathBuf};

use futures_util::StreamExt;
use reqwest::StatusCode;
use reqwest::header::RANGE;
use sha2::{Digest, Sha256};
use tokio::io::AsyncWriteExt;

use crate::catalog::{Model, ModelFile};

/// Progress across all of a model's files.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DownloadProgress {
    pub downloaded_bytes: u64,
    pub total_bytes: u64,
}

#[derive(Debug, thiserror::Error)]
pub enum DownloadError {
    #[error("download request failed: {0}")]
    Request(#[from] reqwest::Error),
    #[error("server answered {status} for {url}")]
    UnexpectedStatus { url: String, status: u16 },
    #[error("could not write the model file: {0}")]
    Io(#[from] std::io::Error),
    #[error(
        "{file_name} failed verification (expected sha256 {expected}, got {actual}); the partial file was deleted"
    )]
    HashMismatch {
        file_name: String,
        expected: String,
        actual: String,
    },
}

/// Downloads every file of `model` into `<models_dir>/<model.id>/` and returns that folder.
/// Dropping the future cancels the download and keeps the partial file for the next call.
pub async fn download_model(
    client: &reqwest::Client,
    model: &Model,
    models_dir: &Path,
    mut on_progress: impl FnMut(DownloadProgress),
) -> Result<PathBuf, DownloadError> {
    let model_dir = models_dir.join(model.id);
    tokio::fs::create_dir_all(&model_dir).await?;
    let total_bytes: u64 = model.files.iter().map(|file| file.bytes).sum();
    let mut finished_bytes = 0;
    for file in model.files {
        download_file(client, file, &model_dir, |file_bytes| {
            on_progress(DownloadProgress {
                downloaded_bytes: finished_bytes + file_bytes,
                total_bytes,
            })
        })
        .await?;
        finished_bytes += file.bytes;
        on_progress(DownloadProgress {
            downloaded_bytes: finished_bytes,
            total_bytes,
        });
    }
    Ok(model_dir)
}

async fn download_file(
    client: &reqwest::Client,
    file: &ModelFile,
    model_dir: &Path,
    mut on_file_progress: impl FnMut(u64),
) -> Result<(), DownloadError> {
    let final_path = model_dir.join(file.name);
    if tokio::fs::try_exists(&final_path).await? {
        return Ok(());
    }
    let partial_path = model_dir.join(format!("{}.part", file.name));
    let resume_from = match tokio::fs::metadata(&partial_path).await {
        Ok(metadata) => metadata.len(),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => 0,
        Err(error) => return Err(error.into()),
    };
    // A partial file already at full size (killed before the rename) needs no request; asking
    // for `bytes=<size>-` would get a 416.
    if resume_from < file.bytes {
        fetch_into_partial_file(client, file, &partial_path, resume_from, &mut on_file_progress)
            .await?;
    }
    let path_to_hash = partial_path.clone();
    let actual_sha256 = tokio::task::spawn_blocking(move || sha256_of_file(&path_to_hash))
        .await
        .expect("hashing task panicked")?;
    if actual_sha256 != file.sha256 {
        tokio::fs::remove_file(&partial_path).await?;
        return Err(DownloadError::HashMismatch {
            file_name: file.name.to_string(),
            expected: file.sha256.to_string(),
            actual: actual_sha256,
        });
    }
    tokio::fs::rename(&partial_path, &final_path).await?;
    Ok(())
}

async fn fetch_into_partial_file(
    client: &reqwest::Client,
    file: &ModelFile,
    partial_path: &Path,
    resume_from: u64,
    on_file_progress: &mut impl FnMut(u64),
) -> Result<(), DownloadError> {
    let mut request = client.get(file.url);
    if resume_from > 0 {
        request = request.header(RANGE, format!("bytes={resume_from}-"));
    }
    let response = request.send().await?;
    // 200 to a Range request means the server ignored it and is sending the whole file,
    // so the partial file must start over rather than be appended to.
    let mut written_bytes = match response.status() {
        StatusCode::PARTIAL_CONTENT => resume_from,
        StatusCode::OK => 0,
        status => {
            return Err(DownloadError::UnexpectedStatus {
                url: file.url.to_string(),
                status: status.as_u16(),
            });
        }
    };
    let mut partial_file = tokio::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .append(written_bytes > 0)
        .truncate(written_bytes == 0)
        .open(partial_path)
        .await?;
    on_file_progress(written_bytes);
    let mut body = response.bytes_stream();
    while let Some(chunk) = body.next().await {
        let chunk = chunk?;
        partial_file.write_all(&chunk).await?;
        written_bytes += chunk.len() as u64;
        on_file_progress(written_bytes);
    }
    // tokio hands writes to a background thread; flush waits for them before the file is hashed.
    partial_file.flush().await?;
    Ok(())
}

fn sha256_of_file(path: &Path) -> std::io::Result<String> {
    let mut file = std::fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0u8; 1024 * 1024];
    loop {
        let read_bytes = file.read(&mut buffer)?;
        if read_bytes == 0 {
            break;
        }
        hasher.update(&buffer[..read_bytes]);
    }
    Ok(hex::encode(hasher.finalize()))
}
```

In `crates/dot-models/src/lib.rs` add `mod download;` beside the other modules and the re-export:

```rust
pub use download::{DownloadError, DownloadProgress, download_model};
```

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test -p dot-models`
Expected: `catalog` `4 passed; 1 ignored`, `download` `8 passed; 1 ignored`, `recommend` `6 passed`.

- [ ] **Step 6: Run the network checks against the real catalog**

Run: `cargo test -p dot-models -- --ignored`
Expected: `every_catalog_url_serves_the_catalog_size ... ok` and `silero_vad_downloads_and_verifies ... ok`. A failure here means a catalog URL, size or hash is wrong: fix the catalog entry from the upstream repo, never the test.

- [ ] **Step 7: Update the docs**

Replace `crates/dot-models/AGENTS.md` with:

```markdown
# dot-models

Owns: the compiled-in catalog of downloadable models (URLs pinned to upstream commits, sha256, sizes), the hardware-based recommendation of which chat model to use, and downloading a model's files into a models folder.

Must not know about: settings, the UI, Tauri, llama-server or any inference. It does not decide where the models folder is; the caller passes it (the app uses `~/Library/Application Support/Hey Dot/models/`).

Entry points: `CATALOG` and the per-model statics (`QWEN3_VL_4B`, `PARAKEET_TDT_V3`, ...); `detect_hardware()` then `recommend(&hardware)`; `download_model(&client, &model, models_dir, on_progress)`.

Invariants and gotchas:
- A file exists at `<models_dir>/<model id>/<file name>` only after its sha256 matched. Anything unverified is `<file name>.part`. "The final file exists" therefore means "installed"; nothing re-hashes installed files.
- A failed hash deletes the `.part` and returns `HashMismatch`; calling again starts that file from zero.
- Dropping the `download_model` future cancels it and keeps the `.part`; the next call resumes with a Range request. A server answering 200 instead of 206 restarts the file.
- One download per model at a time. Two concurrent calls on the same model write the same `.part`; the caller serialises them.
- `on_progress` fires per network chunk (many per second) and reaches 100% before the final hash, which takes a few seconds on multi-GB files. The caller throttles before forwarding to the UI.
- Every URL is pinned to a commit. Changing a file (new commit, other quant) means updating its sha256 and bytes in the same edit, then running `cargo test -p dot-models -- --ignored`, which checks every URL's size against the network.
- Kokoro is not in the catalog yet: its file set depends on the G2P decision in the dot-tts step.
- Qwen3-VL GGUF compatibility with the pinned llama-server is proven in the dot-runtime step, not here.
- Licenses: Qwen3-VL Apache-2.0, Parakeet weights CC-BY-4.0 (attribution needed in the app's about screen), Silero VAD MIT.
- `recommend` thresholds are the chat models' `min_ram_gb` (4B: 16, 8B: 32), compared in whole GiB rounded down. Cloud is only returned for Intel; onboarding offers cloud to everyone anyway.
- The chip is the architecture the binary was compiled for. An Apple Silicon Mac running the app under Rosetta is seen as Intel.
- No free-disk-space check before downloading; a full disk surfaces as `DownloadError::Io`.

Called by: `app/src-tauri` (from Phase 1 step 3 on).
```

In root `AGENTS.md`, in the `## Test` block, after the `cargo test --workspace` line add:

```
    cargo test --workspace -- --ignored   # network and model-dependent checks, not run in CI
```

- [ ] **Step 8: Format, lint, commit**

```bash
cargo fmt --all
cargo clippy -p dot-models --all-targets -- -D warnings
cargo test -p dot-models
git add crates/dot-models Cargo.lock AGENTS.md
git commit -m "Download catalog models with range resume and sha256 verification"
```
Expected: no clippy warnings; `18 passed; 2 ignored` across the three targets.

---

### Task 6: Whole-workspace verification

**Files:** none changed unless a check fails.

**Interfaces:**
- Consumes: everything above.
- Produces: the test output shown to the user before any push.

- [ ] **Step 1: Run the full CI sequence locally**

```bash
pnpm -C app build
pnpm -C app test
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```
Expected: vitest `1 passed`; fmt prints nothing; clippy no warnings; cargo test shows `dot-settings` 9 passed, `dot-models` 18 passed 2 ignored, `hey-dot` 0 tests, no failures.

- [ ] **Step 2: Confirm the app still bundles with the new workspace members**

Run: `pnpm -C app tauri build --target universal-apple-darwin --bundles app,dmg`
Expected: `Finished 2 bundles` and `target/universal-apple-darwin/release/bundle/dmg/Hey Dot_0.1.0_universal.dmg` exists. (The app does not depend on the new crates yet; this proves the workspace change did not break the build.)

- [ ] **Step 3: Show the user the outputs, then push on approval**

```bash
git push origin rebuild
```
Expected (after approval): push succeeds; CI run on `rebuild` shows `check: success`, `build: success`.
