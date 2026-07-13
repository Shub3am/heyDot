# app

Owns: the Tauri shell (`src-tauri/`) and the React UI (`src/`). Windows, tray, hotkeys and the IPC surface between UI and Rust live here.

Must not know about: model formats, audio processing, provider HTTP details. Those belong in `crates/` and are called from `src-tauri` only.

Entry points: `src-tauri/src/main.rs` -> `hey_dot_lib::run()` in `src-tauri/src/lib.rs`; UI starts at `src/main.tsx`.

Invariants and gotchas:
- `tauri::generate_context!` reads `dist/` at compile time, so `pnpm build` must run before `cargo clippy/test/build` on a fresh checkout.
- Commands registered with `invoke_handler` are allowed for every window by default. Capabilities in `src-tauri/capabilities/` gate plugin commands; a new plugin command needs a permission entry there.
- Bundle identifier `com.shub3am.heydot` is permanent once released; macOS permissions are keyed to it.

Called by: the user (launching the app) and CI (`.github/workflows/ci.yml`).
