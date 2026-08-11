# Phase 1 step 4: dot-screen, a screenshot attached to every typed question Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Every question typed in the chat panel carries a JPEG of the display under the mouse cursor, downscaled so its long edge is at most 1600 px, and the panel says whether the screenshot went along and, when it did not, why.

**Architecture:** A new crate `dot-screen` owns the capture: it checks the Screen Recording permission, finds the display under the cursor, captures it with ScreenCaptureKit's `SCScreenshotManager` while excluding this process's own windows, has ScreenCaptureKit scale it to an aspect-preserving size, and encodes JPEG q85. The app's `ask_text` runs that capture on a blocking thread before it streams, attaches the JPEG to the question, and reports the outcome in the `started` event as `ScreenShare` (`attached`, `permissionNeeded`, `failed`). Any capture failure answers text-only with a visible notice; a missing permission shows a card whose button asks macOS for access and opens the Screen Recording pane.

**Tech Stack:** Rust 1.98 edition 2024, Tauri 2.11.6; `screencapturekit` 10.0.3 (feature `macos_14_0`, Swift bridge built by SwiftPM), `objc2-core-graphics` 0.3.2 (default features), `image` 0.25.10 (`default-features = false, features = ["jpeg"]`), `thiserror` 2.0.20; the app gains `dot-settings` for the default screenshot size. Every API below was compile-checked and run on 2026-09-24 in a scratch crate on this Mac (macOS 26, Command Line Tools only): capture of the display under the cursor on a two-display setup, own-window exclusion, 1600x900 JPEG of about 156 KB, 168 ms cold and about 60 ms warm in release.

**Spec:** `docs/specs/2026-09-23-phase0-phase1-design.md` (sections "dot-screen", "Error states the user sees", "Testing", build order step 4).

## Global Constraints

- Crate name is final: `dot-screen`. It joins the workspace through the existing `members = ["app/src-tauri", "crates/*"]`; no root `Cargo.toml` edit.
- "ScreenCaptureKit `SCScreenshotManager` via the `screencapturekit` crate, display under the mouse cursor, excluding Hey Dot's own windows."
- "Downscale so the long edge is at most 1600 px (setting), encode JPEG q85."
- "Returns a typed error for "permission not granted" so onboarding can explain it; never silently continues without an image." The app never drops the image silently: every text-only answer says why in the panel.
- Error state: "Screen permission denied or expired | Answer text-only, with a card explaining and a button to re-grant".
- `SCScreenshotManager` needs macOS 14 at runtime, so the app's minimum becomes macOS 14.0 (`bundle.macOS.minimumSystemVersion`).
- Every source file starts with a header saying why it exists and what it must not do; it never lists functions.
- Every crate has an `AGENTS.md`: owns, must not know about, entry points, invariants and gotchas, called by. "Called by" lines may name callers added later in this plan; the plan lands as one push.
- Shell setup for every command: `export PATH="$HOME/.cargo/bin:$PATH"`, working directory the repo root.
- `pnpm -C app build` and `scripts/build-llama-server.sh` must have run before any cargo command that builds `hey-dot`.
- Before Task 1: `git tag before-phase1-step4` (local only), the named point to return to.
- No em dashes in any file. Commits: one logical change each, no co-author lines, nothing pushed until the user has seen test output.

## Verified facts the tasks rely on

- `SCDisplay::width()/height()` return points, not pixels, despite the crate doc. Pixel size comes from `SCShareableContentInfo::for_filter(&filter)`: `content_rect()` (points) times `point_pixel_scale()`. Measured: a built-in display reports 1710x1107 points at scale 2, native 3420x2214.
- `SCDisplay::frame()` and `CGEvent::location` share one space: global points, top-left origin. `CGRect::contains_point` is half-open.
- `SCScreenshotManager::capture_image` blocks on a semaphore and needs no run loop; it runs from a test or a blocking thread. It returns exactly the configured width and height. The cursor is drawn by default.
- `CGImageExt::rgba_data()` renders through a `CGContext` into tightly packed rows (`width * 4`), so no row padding reaches the encoder.
- `image`'s `JpegEncoder` accepts `L8` and `Rgb8` only; `Rgba8` returns `Unsupported`. The alpha byte is dropped before encoding.
- A denied permission surfaces from ScreenCaptureKit as an untyped `NoShareableContent(String)`, so `CGPreflightScreenCaptureAccess()` is checked first and gives the typed error. It shows no prompt.
- Linking with only the Command Line Tools installed needs two fixes, both verified: `screencapturekit`'s build script searches for `swiftCompatibility56` only inside Xcode (link error `__swift_FORCE_LOAD_$_swiftCompatibility56`), fixed by a link search path next to `xcrun --find swiftc`; and its `-rpath /usr/lib/swift` is a `rustc-link-arg`, which Cargo applies only to the emitting package, so every final binary that links it aborts with `Library not loaded: @rpath/libswift_Concurrency.dylib` until its own build script adds the rpath. A `rustc-link-search` does propagate to dependents.
- `screencapturekit`'s build script passes `--triple x86_64-apple-macosx13.0` when Cargo targets x86_64, so the universal build works.

## Review Focus

1. A Retina display: sizes arrive in points, so a naive capture comes out at half size (855 px) or at native size (3420 px). Expected: long edge exactly 1600. Pinned by `fit_within_long_edge` tests with native pixel sizes (Task 1) and the ignored real capture asserting a 1600 px long edge (Task 3).
2. A portrait or odd-aspect display, or one already smaller than 1600 px. Expected: aspect kept, never upscaled, no zero-width image. Pinned by the portrait, small-display and extreme-aspect tests (Task 1).
3. Screen Recording permission never granted, denied, or expired after macOS' monthly re-confirm. Expected: the answer still arrives, text-only, with the card and a button. Pinned by `screen_share_of` test (Task 4) and the permission card test (Task 5).
4. Capture fails for any other reason (cursor between displays, ScreenCaptureKit error, encode error). Expected: text-only answer with the reason shown, never a silent drop and never a failed question. Pinned by `screen_share_of` test (Task 4) and the failed-notice test (Task 5).
5. Hey Dot's own window covering the content the user asks about. Expected: the screenshot shows what is behind Hey Dot. Pinned by the manual check in Task 6, which asks the model what the screenshot shows while Hey Dot's window is in front; the test binary in Task 3 owns no windows, so no automated test can see this.

---

### Task 1: dot-screen crate and the downscale size

**Files:**
- Create: `crates/dot-screen/Cargo.toml`
- Create: `crates/dot-screen/src/lib.rs`
- Create: `crates/dot-screen/src/scale.rs`
- Create: `crates/dot-screen/AGENTS.md`
- Modify: `AGENTS.md` (Modules list)

**Interfaces:**
- Produces: `pub(crate) fn fit_within_long_edge(width: u32, height: u32, max_long_edge: u32) -> (u32, u32)` in `scale.rs`, used by Task 3.

- [ ] **Step 1: Tag and scaffold**

```bash
git tag before-phase1-step4
```

`crates/dot-screen/Cargo.toml`:

```toml
[package]
name = "dot-screen"
version = "0.1.0"
edition = "2024"
license = "Apache-2.0"
publish = false

[dependencies]
```

`crates/dot-screen/src/lib.rs`:

```rust
//! Captures the display the user is looking at as a JPEG sized for a vision model.
//! Must not know about models, providers, settings files or the UI; the caller passes the size limit.

mod scale;
```

- [ ] **Step 2: Write the failing tests**

`crates/dot-screen/src/scale.rs`:

```rust
//! Picks the pixel size a screenshot is captured at.
//! Must not touch ScreenCaptureKit; it is plain arithmetic so it can be tested without a screen.

pub(crate) fn fit_within_long_edge(width: u32, height: u32, max_long_edge: u32) -> (u32, u32) {
    (width, height)
}

#[cfg(test)]
mod tests {
    use super::fit_within_long_edge;

    #[test]
    fn a_retina_macbook_air_display_shrinks_to_1600_wide() {
        assert_eq!(fit_within_long_edge(3420, 2214, 1600), (1600, 1036));
    }

    #[test]
    fn a_retina_macbook_pro_display_shrinks_to_1600_wide() {
        assert_eq!(fit_within_long_edge(3024, 1964, 1600), (1600, 1039));
    }

    #[test]
    fn a_portrait_display_shrinks_to_1600_tall() {
        assert_eq!(fit_within_long_edge(1080, 1920, 1600), (900, 1600));
    }

    #[test]
    fn a_display_smaller_than_the_limit_is_never_upscaled() {
        assert_eq!(fit_within_long_edge(1512, 982, 1600), (1512, 982));
    }

    #[test]
    fn a_display_exactly_at_the_limit_is_unchanged() {
        assert_eq!(fit_within_long_edge(1600, 900, 1600), (1600, 900));
    }

    #[test]
    fn an_extreme_aspect_never_rounds_an_edge_to_zero() {
        assert_eq!(fit_within_long_edge(20000, 1, 1600), (1600, 1));
    }
}
```

- [ ] **Step 3: Run the tests to verify they fail**

Run: `cargo test -p dot-screen`
Expected: FAIL, 4 of 6 fail with `left: (3420, 2214)` style mismatches; `a_display_smaller_than_the_limit_is_never_upscaled` and `a_display_exactly_at_the_limit_is_unchanged` already pass with the stub.

- [ ] **Step 4: Implement**

Replace the stub body:

```rust
pub(crate) fn fit_within_long_edge(width: u32, height: u32, max_long_edge: u32) -> (u32, u32) {
    let long_edge = width.max(height);
    if long_edge <= max_long_edge {
        return (width, height);
    }
    let shrink = f64::from(max_long_edge) / f64::from(long_edge);
    let shrink_edge = |edge: u32| ((f64::from(edge) * shrink).round() as u32).max(1);
    (shrink_edge(width), shrink_edge(height))
}
```

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test -p dot-screen`
Expected: PASS, 6 passed, plus a `dead_code` warning for `fit_within_long_edge`: it has no caller outside tests until Task 3, which is where clippy with `-D warnings` first runs on this crate.

- [ ] **Step 6: Module docs**

`crates/dot-screen/AGENTS.md`:

```markdown
# dot-screen

Owns: capturing the display under the mouse cursor as a JPEG for a vision model, with Hey Dot's own windows left out, scaled so its long edge fits the caller's limit, and telling a missing Screen Recording permission apart from every other failure.

Must not know about: models, providers, the settings file or the UI. The caller passes the size limit and decides what to do without an image.

Entry points: `capture_display_under_cursor(max_long_edge_px)`, blocking, returns JPEG bytes or `ScreenError`; `request_screen_access()`.

Invariants and gotchas:
- Never upscales: a display smaller than the limit is captured at its native pixel size. Aspect is always kept.

Called by: `app/src-tauri` (the `ask_text` and `open_screen_recording_settings` commands).
```

Root `AGENTS.md`, after the `crates/dot-providers/` line:

```markdown
- `crates/dot-screen/`: screenshot of the display under the cursor, as JPEG. See `crates/dot-screen/AGENTS.md`.
```

- [ ] **Step 7: Commit**

```bash
git add crates/dot-screen AGENTS.md Cargo.lock
git commit -m "dot-screen: pick the capture size that fits the long-edge limit"
```

---

### Task 2: JPEG encoding and the error type

**Files:**
- Create: `crates/dot-screen/src/jpeg.rs`
- Create: `crates/dot-screen/src/screen_error.rs`
- Modify: `crates/dot-screen/src/lib.rs`, `crates/dot-screen/Cargo.toml`

**Interfaces:**
- Produces: `pub enum ScreenError` (exported), with `EncodeFailed(#[from] image::ImageError)` in this task; `pub(crate) fn encode_jpeg(rgba: &[u8], width: u32, height: u32) -> Result<Vec<u8>, ScreenError>`, used by Task 3.

- [ ] **Step 1: Dependencies and the error type**

`crates/dot-screen/Cargo.toml` `[dependencies]`:

```toml
image = { version = "0.25.10", default-features = false, features = ["jpeg"] }
thiserror = "2.0.20"
```

`crates/dot-screen/src/screen_error.rs`:

```rust
//! Every way a screenshot can fail, with a missing permission kept apart so the app can ask for it.
//! Must not carry UI wording beyond one plain sentence per case.

#[derive(Debug, thiserror::Error)]
pub enum ScreenError {
    #[error("could not encode the screenshot as JPEG: {0}")]
    EncodeFailed(#[from] image::ImageError),
}
```

`lib.rs` gains:

```rust
mod jpeg;
mod scale;
mod screen_error;

pub use screen_error::ScreenError;
```

- [ ] **Step 2: Write the failing test**

`crates/dot-screen/src/jpeg.rs`:

```rust
//! Turns captured RGBA pixels into the JPEG that travels with a question.
//! Must not resize; the capture already has its final size.

use crate::ScreenError;

pub(crate) fn encode_jpeg(rgba: &[u8], width: u32, height: u32) -> Result<Vec<u8>, ScreenError> {
    Ok(Vec::new())
}

#[cfg(test)]
mod tests {
    use super::encode_jpeg;

    #[test]
    fn rgba_pixels_become_a_jpeg_of_the_same_size() {
        let (width, height) = (40, 30);
        let rgba: Vec<u8> = (0..width * height).flat_map(|pixel| [pixel as u8, 90, 200, 255]).collect();

        let jpeg = encode_jpeg(&rgba, width, height).unwrap();

        assert_eq!(&jpeg[..3], &[0xFF, 0xD8, 0xFF]);
        let decoded = image::load_from_memory_with_format(&jpeg, image::ImageFormat::Jpeg).unwrap();
        assert_eq!((decoded.width(), decoded.height()), (width, height));
    }
}
```

- [ ] **Step 3: Run it to verify it fails**

Run: `cargo test -p dot-screen jpeg`
Expected: FAIL, panics on the slice `&jpeg[..3]` of an empty vector (`range end index 3 out of range for slice of length 0`).

- [ ] **Step 4: Implement**

```rust
use image::codecs::jpeg::JpegEncoder;
use image::{ExtendedColorType, ImageEncoder};

use crate::ScreenError;

const JPEG_QUALITY: u8 = 85;

pub(crate) fn encode_jpeg(rgba: &[u8], width: u32, height: u32) -> Result<Vec<u8>, ScreenError> {
    // The JPEG encoder rejects RGBA outright; a screen has no transparency to lose.
    let rgb: Vec<u8> = rgba
        .chunks_exact(4)
        .flat_map(|pixel| [pixel[0], pixel[1], pixel[2]])
        .collect();
    let mut jpeg = Vec::new();
    JpegEncoder::new_with_quality(&mut jpeg, JPEG_QUALITY).write_image(
        &rgb,
        width,
        height,
        ExtendedColorType::Rgb8,
    )?;
    Ok(jpeg)
}
```

- [ ] **Step 5: Run it to verify it passes**

Run: `cargo test -p dot-screen`
Expected: PASS, 7 passed, plus the same `dead_code` warning for `encode_jpeg` until Task 3.

- [ ] **Step 6: Commit**

```bash
git add crates/dot-screen Cargo.lock
git commit -m "dot-screen: encode captured pixels as JPEG q85"
```

---

### Task 3: capture the display under the cursor

**Files:**
- Create: `crates/dot-screen/build.rs`
- Create: `crates/dot-screen/src/capture.rs`
- Create: `crates/dot-screen/tests/capture_display.rs`
- Modify: `crates/dot-screen/src/lib.rs`, `crates/dot-screen/src/screen_error.rs`, `crates/dot-screen/Cargo.toml`, `crates/dot-screen/AGENTS.md`

**Interfaces:**
- Consumes: `fit_within_long_edge` (Task 1), `encode_jpeg` (Task 2).
- Produces: `pub fn capture_display_under_cursor(max_long_edge_px: u32) -> Result<Vec<u8>, ScreenError>`; `pub fn request_screen_access()`; `ScreenError` variants `PermissionNotGranted`, `NoDisplayUnderCursor`, `CaptureFailed(#[from] SCError)`, `EncodeFailed`.

- [ ] **Step 1: Dependencies and the link workaround**

`crates/dot-screen/Cargo.toml` `[dependencies]` becomes:

```toml
image = { version = "0.25.10", default-features = false, features = ["jpeg"] }
objc2-core-graphics = "0.3.2"
screencapturekit = { version = "10.0.3", features = ["macos_14_0"] }
thiserror = "2.0.20"
```

`crates/dot-screen/build.rs`:

```rust
//! Links screencapturekit's Swift bridge on Macs that have only the Command Line Tools.
//! Must not build anything itself; screencapturekit's own build script builds the bridge.

use std::path::Path;
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-env-changed=DEVELOPER_DIR");

    // screencapturekit looks for the Swift compatibility libraries inside Xcode only. Without Xcode
    // they sit next to swiftc, and the link fails on `__swift_FORCE_LOAD_$_swiftCompatibility56`.
    // A link search path reaches every crate that depends on this one.
    let swiftc = Command::new("xcrun")
        .args(["--find", "swiftc"])
        .output()
        .expect("xcrun ships with the macOS developer tools");
    let swiftc = String::from_utf8(swiftc.stdout).expect("xcrun prints a UTF-8 path");
    let swift_libraries = Path::new(swiftc.trim())
        .parent()
        .expect("swiftc sits in a bin folder")
        .join("../lib/swift/macosx");
    println!("cargo:rustc-link-search=native={}", swift_libraries.display());

    // Link args reach only the package that prints them, so screencapturekit's own rpath never
    // reaches these tests. app/src-tauri/build.rs repeats this line for the app.
    println!("cargo:rustc-link-arg=-Wl,-rpath,/usr/lib/swift");
}
```

- [ ] **Step 2: Write the failing ignored test**

`crates/dot-screen/tests/capture_display.rs`:

```rust
// Needs a real screen and Screen Recording permission for the terminal, so it is ignored in CI.
// Every display this runs on is at least 1600 px on its long edge (any Retina Mac, any 1080p screen).

use std::time::Instant;

use dot_screen::capture_display_under_cursor;

#[test]
#[ignore = "needs a screen and Screen Recording permission"]
fn captures_the_display_under_the_cursor_at_1600_px_on_its_long_edge() {
    for attempt in 1..=5 {
        let started = Instant::now();
        let jpeg = capture_display_under_cursor(1600).unwrap();
        let elapsed = started.elapsed();

        let decoded = image::load_from_memory_with_format(&jpeg, image::ImageFormat::Jpeg).unwrap();
        println!(
            "capture {attempt}: {}x{}, {} KB, {elapsed:?}",
            decoded.width(),
            decoded.height(),
            jpeg.len() / 1024
        );
        assert_eq!(decoded.width().max(decoded.height()), 1600);
    }
}
```

Integration tests see the package's own `[dependencies]`, so `image` needs no dev-dependency entry.

`lib.rs`: add `mod capture;` and `pub use capture::{capture_display_under_cursor, request_screen_access};`, with a stub `capture.rs`:

```rust
//! Takes one screenshot of the display under the mouse cursor, without Hey Dot's own windows.
//! Must not pick the size limit or decide what happens without a screenshot; the caller does both.

use crate::ScreenError;

/// Blocks for about 60 ms warm, 170 ms on the first capture. Run it off the async runtime.
pub fn capture_display_under_cursor(max_long_edge_px: u32) -> Result<Vec<u8>, ScreenError> {
    Ok(Vec::new())
}

pub fn request_screen_access() {}
```

- [ ] **Step 3: Run it to verify it fails**

Run: `cargo test --release -p dot-screen --test capture_display -- --ignored --nocapture`
Expected: it compiles and links (this proves the build script), then FAILS in `load_from_memory_with_format` on the empty vector (`unwrap()` on an `Err` from the JPEG decoder).

- [ ] **Step 4: Implement**

`screen_error.rs` becomes:

```rust
use screencapturekit::error::SCError;

#[derive(Debug, thiserror::Error)]
pub enum ScreenError {
    #[error("Hey Dot does not have Screen Recording permission")]
    PermissionNotGranted,
    #[error("no display is under the mouse cursor")]
    NoDisplayUnderCursor,
    #[error("ScreenCaptureKit failed: {0}")]
    CaptureFailed(#[from] SCError),
    #[error("could not encode the screenshot as JPEG: {0}")]
    EncodeFailed(#[from] image::ImageError),
}
```

(keep the file header from Task 2 above it).

`capture.rs`:

```rust
//! Takes one screenshot of the display under the mouse cursor, without Hey Dot's own windows.
//! Must not pick the size limit or decide what happens without a screenshot; the caller does both.

use objc2_core_graphics::{CGEvent, CGPreflightScreenCaptureAccess, CGRequestScreenCaptureAccess};
use screencapturekit::cg::CGPoint;
use screencapturekit::error::SCError;
use screencapturekit::screenshot_manager::{CGImageExt, SCScreenshotManager};
use screencapturekit::shareable_content::{SCShareableContent, SCShareableContentInfo, SCWindow};
use screencapturekit::stream::configuration::SCStreamConfiguration;
use screencapturekit::stream::content_filter::SCContentFilter;

use crate::ScreenError;
use crate::jpeg::encode_jpeg;
use crate::scale::fit_within_long_edge;

/// Blocks for about 60 ms warm, 170 ms on the first capture. Run it off the async runtime.
pub fn capture_display_under_cursor(max_long_edge_px: u32) -> Result<Vec<u8>, ScreenError> {
    // ScreenCaptureKit reports a denied permission as an untyped "no shareable content" error.
    if !CGPreflightScreenCaptureAccess() {
        return Err(ScreenError::PermissionNotGranted);
    }
    let cursor_event = CGEvent::new(None);
    let cursor = CGEvent::location(cursor_event.as_deref());
    let content = SCShareableContent::get()?;
    // Display frames and the cursor are both in global points with a top-left origin.
    let display = content
        .displays()
        .into_iter()
        .find(|display| display.frame().contains_point(CGPoint::new(cursor.x, cursor.y)))
        .ok_or(ScreenError::NoDisplayUnderCursor)?;

    let own_pid = std::process::id() as i32;
    let windows = content.windows();
    let own_windows: Vec<&SCWindow> = windows
        .iter()
        .filter(|window| {
            window
                .owning_application()
                .is_some_and(|app| app.process_id() == own_pid)
        })
        .collect();
    let filter = SCContentFilter::create()
        .with_display(&display)
        .with_excluding_windows(&own_windows)
        .build();

    // SCDisplay's width and height are points; the pixel size needs the display's scale.
    let filter_info = SCShareableContentInfo::for_filter(&filter).ok_or_else(|| {
        SCError::NoShareableContent("no size information for the display".to_owned())
    })?;
    let scale = f64::from(filter_info.point_pixel_scale());
    let points = filter_info.content_rect().size;
    let (width, height) = fit_within_long_edge(
        (points.width * scale).round() as u32,
        (points.height * scale).round() as u32,
        max_long_edge_px,
    );
    let config = SCStreamConfiguration::new()
        .with_width(width)
        .with_height(height);
    let image = SCScreenshotManager::capture_image(&filter, &config)?;
    encode_jpeg(&image.rgba_data()?, image.width() as u32, image.height() as u32)
}

/// Shows macOS' Screen Recording prompt the first time and lists Hey Dot in System Settings.
/// After the user has answered once, it does nothing.
pub fn request_screen_access() {
    CGRequestScreenCaptureAccess();
}
```

If `SCError::NoShareableContent` does not take a `String` in 10.0.3, check `src/error.rs` in the crate and use the variant that carries a message; ledger the ruling.

- [ ] **Step 5: Run it to verify it passes, and record the benchmark**

Run: `cargo test --release -p dot-screen --test capture_display -- --ignored --nocapture`
Expected: PASS, five lines like `capture 1: 1600x1036, 150 KB, 170ms` then about 60 ms each. Paste the output in the task's ledger line and in the final report; this is the capture benchmark.

Run: `cargo test -p dot-screen && cargo clippy -p dot-screen --all-targets -- -D warnings`
Expected: 7 unit tests pass, the capture test is ignored, clippy clean.

- [ ] **Step 6: Module docs**

Append to the invariants in `crates/dot-screen/AGENTS.md`:

```markdown
- Needs macOS 14: `SCScreenshotManager` does not exist earlier.
- `SCDisplay::width()/height()` are points despite the crate doc. The pixel size is `content_rect` times `point_pixel_scale` from `SCShareableContentInfo`.
- The permission check is `CGPreflightScreenCaptureAccess`, because ScreenCaptureKit reports a denial as an untyped "no shareable content" error. It shows no prompt; `request_screen_access` does, once.
- macOS keys the permission to the process that asks: the bundled app for users, the terminal for `cargo test`. A granted permission takes effect after the app restarts.
- Own windows are found by this process's pid, so the exclusion covers every Hey Dot window without naming any.
- `capture_display_under_cursor` blocks (about 60 ms warm, 170 ms cold in release). Async callers use `spawn_blocking`.
- `build.rs` adds a Swift library search path for Macs without Xcode and an `-rpath /usr/lib/swift`. Link args do not propagate, so every binary that links this crate repeats the rpath in its own build script, or it aborts at launch on `libswift_Concurrency.dylib`.
- The real capture test is ignored in CI: `cargo test --release -p dot-screen --test capture_display -- --ignored --nocapture`, with Screen Recording allowed for the terminal. It prints the timing of five captures.
```

- [ ] **Step 7: Commit**

```bash
git add crates/dot-screen Cargo.lock
git commit -m "dot-screen: capture the display under the cursor without Hey Dot's windows"
```

---

### Task 4: attach the screenshot in ask_text

**Files:**
- Modify: `app/src-tauri/Cargo.toml`, `app/src-tauri/build.rs`, `app/src-tauri/src/commands.rs`, `app/src-tauri/src/lib.rs`, `app/src-tauri/tauri.conf.json`, `app/AGENTS.md`, `crates/dot-settings/AGENTS.md`, `README.md`

**Interfaces:**
- Consumes: `dot_screen::{capture_display_under_cursor, request_screen_access, ScreenError}` (Task 3); `dot_settings::Settings::default().screenshot_max_edge_px` (existing, 1600).
- Produces (the IPC contract Task 5 mirrors):
  - `AnswerEvent::Started { leaves_device: bool, host: String, screen: ScreenShare }`, JSON `{"event":"started","data":{"leavesDevice":false,"host":"127.0.0.1","screen":{"kind":"attached"}}}`.
  - `ScreenShare` JSON: `{"kind":"attached"}`, `{"kind":"permissionNeeded"}`, `{"kind":"failed","reason":"<text>"}`.
  - Command `open_screen_recording_settings`, no arguments, resolves to nothing or rejects with the error text.

- [ ] **Step 1: Dependencies and the rpath**

`app/src-tauri/Cargo.toml` `[dependencies]` gains, in the existing order style:

```toml
dot-screen = { path = "../../crates/dot-screen" }
dot-settings = { path = "../../crates/dot-settings" }
```

`app/src-tauri/build.rs`:

```rust
fn main() {
    // dot-screen's Swift bridge needs the system Swift runtime at launch; its own rpath link arg
    // does not reach this binary, so the app would abort on libswift_Concurrency.dylib.
    println!("cargo:rustc-link-arg=-Wl,-rpath,/usr/lib/swift");
    tauri_build::build()
}
```

- [ ] **Step 2: Write the failing tests**

In `commands.rs` tests, replace the `started` value in `answer_events_serialize_in_the_shape_the_chat_panel_reads` and add three tests:

```rust
    #[test]
    fn answer_events_serialize_in_the_shape_the_chat_panel_reads() {
        let started = AnswerEvent::Started {
            leaves_device: false,
            host: "127.0.0.1".to_owned(),
            screen: ScreenShare::Attached,
        };
        assert_eq!(
            serde_json::to_value(&started).unwrap(),
            serde_json::json!({"event": "started", "data": {"leavesDevice": false, "host": "127.0.0.1", "screen": {"kind": "attached"}}})
        );
        assert_eq!(
            serde_json::to_value(AnswerEvent::Delta {
                text: "Par".to_owned()
            })
            .unwrap(),
            serde_json::json!({"event": "delta", "data": {"text": "Par"}})
        );
    }

    #[test]
    fn screen_shares_serialize_in_the_shape_the_chat_panel_reads() {
        assert_eq!(
            serde_json::to_value(ScreenShare::PermissionNeeded).unwrap(),
            serde_json::json!({"kind": "permissionNeeded"})
        );
        assert_eq!(
            serde_json::to_value(ScreenShare::Failed {
                reason: "no display is under the mouse cursor".to_owned()
            })
            .unwrap(),
            serde_json::json!({"kind": "failed", "reason": "no display is under the mouse cursor"})
        );
    }

    #[test]
    fn a_missing_permission_asks_for_it() {
        assert_eq!(
            screen_share_of(&Err(ScreenError::PermissionNotGranted)),
            ScreenShare::PermissionNeeded
        );
    }

    #[test]
    fn any_other_capture_failure_shows_its_reason() {
        assert_eq!(
            screen_share_of(&Err(ScreenError::NoDisplayUnderCursor)),
            ScreenShare::Failed {
                reason: "no display is under the mouse cursor".to_owned()
            }
        );
    }
```

- [ ] **Step 3: Run them to verify they fail**

Run: `cargo test -p hey-dot --lib commands`
Expected: FAIL to compile: `cannot find type ScreenShare`, `cannot find function screen_share_of`, `no field screen on AnswerEvent::Started`.

- [ ] **Step 4: Implement**

`commands.rs` imports gain `use dot_screen::{ScreenError, capture_display_under_cursor, request_screen_access};` and `use dot_settings::Settings;`. The enum and new items:

```rust
pub enum AnswerEvent {
    Started {
        leaves_device: bool,
        host: String,
        screen: ScreenShare,
    },
    Delta {
        text: String,
    },
}

/// Whether the question's screenshot went along, and if not, why.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase", rename_all_fields = "camelCase", tag = "kind")]
pub enum ScreenShare {
    Attached,
    PermissionNeeded,
    Failed { reason: String },
}

fn screen_share_of(capture: &Result<Vec<u8>, ScreenError>) -> ScreenShare {
    match capture {
        Ok(_) => ScreenShare::Attached,
        Err(ScreenError::PermissionNotGranted) => ScreenShare::PermissionNeeded,
        Err(error) => ScreenShare::Failed {
            reason: error.to_string(),
        },
    }
}
```

`ask_text` body, from after `chat_config()` to the stream:

```rust
    // Read from the defaults until the settings window exists; the file is not loaded yet.
    let max_long_edge_px = Settings::default().screenshot_max_edge_px;
    let screenshot = tauri::async_runtime::spawn_blocking(move || {
        capture_display_under_cursor(max_long_edge_px)
    })
    .await
    .map_err(|error| error.to_string())?;
    let started = AnswerEvent::Started {
        leaves_device: config.leaves_device(),
        host: config.base_url.host_str().unwrap_or_default().to_owned(),
        screen: screen_share_of(&screenshot),
    };
    on_event.send(started).map_err(|error| error.to_string())?;
    let messages = [ChatMessage {
        role: ChatRole::User,
        text: question,
        jpeg_image: screenshot.ok(),
    }];
```

The new command, after `ask_text`:

```rust
/// The permission card's button. macOS only lists an app under Screen Recording after it asked
/// once, so this asks first, then opens the pane where the user turns Hey Dot on.
#[tauri::command]
pub fn open_screen_recording_settings() -> Result<(), String> {
    request_screen_access();
    std::process::Command::new("open")
        .arg("x-apple.systempreferences:com.apple.preference.security?Privacy_ScreenCapture")
        .spawn()
        .map(drop)
        .map_err(|error| error.to_string())
}
```

`lib.rs` `generate_handler!` gains `commands::open_screen_recording_settings` after `commands::ask_text`.

`tauri.conf.json` `bundle` gains:

```json
    "macOS": {
      "minimumSystemVersion": "14.0"
    },
```

- [ ] **Step 5: Run them to verify they pass, then the Rust suite**

Run: `pnpm -C app build && cargo test -p hey-dot`
Expected: PASS, the 4 command tests plus every existing `hey-dot` test.

Run: `cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace`
Expected: all clean and passing.

- [ ] **Step 6: Docs**

`app/AGENTS.md` invariants: replace the `AnswerEvent and LocalModelStatus` line and add four:

```markdown
- The `AnswerEvent` (with `ScreenShare`) and `LocalModelStatus` serde shapes are a contract with `src/chat/ipc.ts`; the serialization tests in `commands.rs` and `local_model.rs` pin them.
- `ask_text` captures the display under the cursor before it answers. A failed capture never fails the question: the answer comes text-only and `ScreenShare` tells the panel why.
- The screenshot size is `Settings::default().screenshot_max_edge_px` until the settings file is loaded at startup (settings window, Phase 1 step 8).
- `build.rs` adds `-rpath /usr/lib/swift` for dot-screen's Swift bridge; without it the app aborts at launch on `libswift_Concurrency.dylib`.
- Minimum macOS is 14.0 (`bundle.macOS.minimumSystemVersion`) because dot-screen uses `SCScreenshotManager`. Screen Recording permission belongs to `com.shub3am.heydot` in a bundle and to the terminal under `tauri dev`.
```

`crates/dot-settings/AGENTS.md`, the stale "Called by" line (the app did not depend on this crate in step 3) becomes:

```markdown
Called by: `app/src-tauri`, which reads `Settings::default()` for the screenshot size (from Phase 1 step 4). Loading the file and the Keychain come with the settings window.
```

`README.md` line 16 becomes:

```markdown
Requirements: macOS 14 or later, Rust (via rustup; the pinned version installs itself), Node 26, pnpm 12.
```

- [ ] **Step 7: Commit, in two logical changes**

```bash
git add app/src-tauri/Cargo.toml app/src-tauri/build.rs app/src-tauri/src/commands.rs app/src-tauri/src/lib.rs app/AGENTS.md crates/dot-settings/AGENTS.md Cargo.lock
git commit -m "app: attach a screenshot of the display under the cursor to every question"
git add app/src-tauri/tauri.conf.json README.md
git commit -m "app: require macOS 14 for ScreenCaptureKit screenshots"
```

---

### Task 5: the panel says whether the screenshot went along

**Files:**
- Modify: `app/src/chat/ipc.ts`, `app/src/chat/ChatPanel.tsx`, `app/src/chat/ChatPanel.test.tsx`

**Interfaces:**
- Consumes: the `started` event shape and `open_screen_recording_settings` command from Task 4.
- Produces: `export type ScreenShare`, `export function openScreenRecordingSettings(): Promise<void>` in `ipc.ts`.

- [ ] **Step 1: Types**

`ipc.ts`:

```ts
export type ScreenShare =
  | { kind: "attached" }
  | { kind: "permissionNeeded" }
  | { kind: "failed"; reason: string };

export type AnswerEvent =
  | { event: "started"; data: { leavesDevice: boolean; host: string; screen: ScreenShare } }
  | { event: "delta"; data: { text: string } };
```

and after `askText`:

```ts
export function openScreenRecordingSettings(): Promise<void> {
  return invoke("open_screen_recording_settings");
}
```

- [ ] **Step 2: Write the failing tests**

In `ChatPanel.test.tsx`, import `ScreenShare` from `./ipc`, add a helper under `askButton`:

```ts
function started(screenShare: ScreenShare, leavesDevice = false, host = "127.0.0.1"): AnswerEvent {
  return { event: "started", data: { leavesDevice, host, screen: screenShare } };
}
```

Replace the two existing `started` literals: `onEvent.onmessage(started({ kind: "attached" }));` in the on-this-mac test, and `onEvent.onmessage(started({ kind: "attached" }, true, "api.openai.com"));` in the leaves-the-Mac test. Add:

```ts
test("an answer with a screenshot says so", async () => {
  await renderPanel(async (_question, onEvent) => {
    onEvent.onmessage(started({ kind: "attached" }));
  });
  sendPhase({ kind: "ready" });
  typeQuestion("What is on my screen?");
  fireEvent.click(askButton());
  expect(await screen.findByText("With a screenshot of this screen")).toBeTruthy();
});

test("a missing screen permission answers without a screenshot and offers the settings", async () => {
  await renderPanel(async (_question, onEvent) => {
    onEvent.onmessage(started({ kind: "permissionNeeded" }));
  });
  sendPhase({ kind: "ready" });
  typeQuestion("What is on my screen?");
  fireEvent.click(askButton());
  expect((await screen.findByRole("status")).textContent).toContain("needs Screen Recording permission");
  fireEvent.click(screen.getByRole("button", { name: "Open Screen Recording settings" }));
  await waitFor(() => expect(invokedCommands).toContain("open_screen_recording_settings"));
});

test("a failed screenshot says why the answer has none", async () => {
  await renderPanel(async (_question, onEvent) => {
    onEvent.onmessage(started({ kind: "failed", reason: "no display is under the mouse cursor" }));
  });
  sendPhase({ kind: "ready" });
  typeQuestion("What is on my screen?");
  fireEvent.click(askButton());
  expect((await screen.findByRole("status")).textContent).toContain(
    "Answered without a screenshot: no display is under the mouse cursor",
  );
});
```

- [ ] **Step 3: Run them to verify they fail**

Run: `pnpm -C app test`
Expected: FAIL, the three new tests fail (`Unable to find an element with the text: With a screenshot of this screen`, `Unable to find role="status"` twice); the 12 existing tests and `App.test.tsx` pass. `pnpm -C app build` (tsc) passes because the new field is already in `ipc.ts`.

- [ ] **Step 4: Implement**

`ChatPanel.tsx`: import `openScreenRecordingSettings` and `type ScreenShare`; `Answer` gains `screen: ScreenShare | null`; `setAnswer({ question, text: "", badge: null, screen: null, error: null })`; the `started` branch returns `{ ...current, badge, screen: event.data.screen }`. Add below `describeModel`:

```tsx
function describeScreenShare(screen: ScreenShare) {
  switch (screen.kind) {
    case "attached":
      return <p>With a screenshot of this screen</p>;
    case "permissionNeeded":
      return (
        <p role="status">
          Answered without a screenshot: Hey Dot needs Screen Recording permission. Turn on Hey Dot in System
          Settings, then quit and reopen Hey Dot.{" "}
          <button onClick={() => void openScreenRecordingSettings()}>Open Screen Recording settings</button>
        </p>
      );
    case "failed":
      return <p role="status">Answered without a screenshot: {screen.reason}</p>;
  }
}
```

and in the `<article>`, after the badge line:

```tsx
          {answer.screen && describeScreenShare(answer.screen)}
```

- [ ] **Step 5: Run them to verify they pass**

Run: `pnpm -C app build && pnpm -C app test`
Expected: PASS, 15 ChatPanel tests plus `App.test.tsx`.

- [ ] **Step 6: Commit**

```bash
git add app/src/chat
git commit -m "chat panel: show whether the screenshot went along and how to allow it"
```

---

### Task 6: full suite, benchmark and a real click-through

**Files:** none changed unless a check fails (then fix under TDD in the owning task's files, and ledger it).

- [ ] **Step 1: Full suite**

Run: `pnpm -C app build && pnpm -C app test && cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace`
Expected: all pass. Paste the tail of each.

- [ ] **Step 2: Capture benchmark**

Run: `cargo test --release -p dot-screen --test capture_display -- --ignored --nocapture`
Expected: five captures at 1600 px long edge; paste the timings.

- [ ] **Step 3: Bundled debug app**

Run: `pnpm -C app tauri build --debug --bundles app`
Then launch `target/debug/bundle/macos/Hey Dot.app` with `open`. Expected: it starts (no `libswift_Concurrency.dylib` abort; check with `log show --last 2m --predicate 'process == "hey-dot"'` if it vanishes).

With the model ready, ask "What is on my screen?" and read the panel:
- The bundle has no Screen Recording permission yet: expected the permission card. Click "Open Screen Recording settings": expected the Screen Recording pane opens with Hey Dot listed. Ask the user to turn it on (a permission change is theirs to make), then quit and reopen.
- With permission: put a window with known text (for example a TextEdit document reading "Blue giraffe 42") behind Hey Dot's window on the same display, ask "What text do you see on my screen?". Expected: "With a screenshot of this screen", and the answer names the TextEdit text and does not describe Hey Dot's own chat panel (Review Focus 5).
- `NSScreenCaptureUsageDescription`: check whether macOS showed its prompt without it. Add the key to `Info.plist` only if the prompt or capture fails without it; ledger what happened either way.

The window is driven by `open`, `screencapture -l <window id>` and AppleScript/System Events keystrokes, as in step 3; ask the user which browser-automation driver to use only if a web page ever enters this check (none does).

- [ ] **Step 4: Ledger and report**

Record the suite output, benchmark and click-through results in the ledger. Nothing is pushed until the user has seen them.
