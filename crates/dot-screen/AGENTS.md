# dot-screen

Owns: capturing the display under the mouse cursor as a JPEG for a vision model, with Hey Dot's own windows left out, scaled so its long edge fits the caller's limit, and telling a missing Screen Recording permission apart from every other failure.

Must not know about: models, providers, the settings file or the UI. The caller passes the size limit and decides what to do without an image.

Entry points: `capture_display_under_cursor(max_long_edge_px)`, blocking, returns JPEG bytes or `ScreenError`; `request_screen_access()`.

Invariants and gotchas:
- Never upscales: a display smaller than the limit is captured at its native pixel size. Aspect is always kept.
- Needs macOS 14: `SCScreenshotManager` does not exist earlier.
- `SCDisplay::width()/height()` are points despite the crate doc. The pixel size is `content_rect` times `point_pixel_scale` from `SCShareableContentInfo`.
- The permission check is `CGPreflightScreenCaptureAccess`, because ScreenCaptureKit reports a denial as an untyped "no shareable content" error. It shows no prompt; `request_screen_access` does, once.
- macOS keys the permission to the process that asks: the bundled app for users, the terminal for `cargo test`. A granted permission takes effect after the app restarts.
- Own windows are found by this process's pid, so the exclusion covers every Hey Dot window without naming any.
- `capture_display_under_cursor` blocks (about 95 ms warm, 165 ms cold in release). Async callers use `spawn_blocking`.
- `build.rs` adds a Swift library search path for Macs without Xcode and an `-rpath /usr/lib/swift`. Link args do not propagate, so every binary that links this crate repeats the rpath in its own build script, or it aborts at launch on `libswift_Concurrency.dylib`.
- The real capture test is ignored in CI: `cargo test --release -p dot-screen --test capture_display -- --ignored --nocapture`, with Screen Recording allowed for the terminal. It prints the timing of five captures.

Called by: `app/src-tauri` (the `ask_text` and `open_screen_recording_settings` commands).
