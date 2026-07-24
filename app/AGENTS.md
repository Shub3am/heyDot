# app

Owns: the Tauri shell (`src-tauri/`) and the React UI (`src/`). Windows, tray, hotkeys and the IPC surface between UI and Rust live here.

Must not know about: model formats, audio processing, provider HTTP details. Those belong in `crates/` and are called from `src-tauri` only.

Entry points: `src-tauri/src/main.rs` -> `hey_dot_lib::run()` in `src-tauri/src/lib.rs`; UI starts at `src/main.tsx`. IPC commands live in `src-tauri/src/commands.rs` and their TypeScript mirror in `src/chat/ipc.ts`.

Invariants and gotchas:
- `tauri::generate_context!` reads `dist/` at compile time, so `pnpm build` must run before `cargo clippy/test/build` on a fresh checkout.
- Commands registered with `invoke_handler` are allowed for every window by default. Capabilities in `src-tauri/capabilities/` gate plugin commands; a new plugin command needs a permission entry there.
- Bundle identifier `com.shub3am.heydot` is permanent once released; macOS permissions are keyed to it.
- `scripts/build-llama-server.sh` must have run before any cargo command on `hey-dot`: tauri-build fails the build while `src-tauri/binaries/llama-server-<target triple>` or `src-tauri/licenses/llama.cpp/` is missing. Both folders are git-ignored build output.
- The bundled llama-server sits next to the app's executable (`Contents/MacOS/` in the bundle, `target/debug/` in `tauri dev`), which is how `lib.rs` finds it.
- `RunEvent::Exit` stops llama-server with `block_on`: Tauri ends with `process::exit`, which skips destructors, so `kill_on_drop` alone never fires.
- The `AnswerEvent` and `LocalModelStatus` serde shapes are a contract with `src/chat/ipc.ts`; the serialization tests in `commands.rs` and `local_model.rs` pin them.
- Two HTTP clients: downloads follow the system proxy, while `local_model_http_client()` (managed state, used by `ask_text`) skips it so prompts and the llama-server key never reach a proxy. `tests/local_model_http_client.rs` pins it.
- Paths the app owns: models in `~/Library/Application Support/Hey Dot/models/`, llama-server log in `~/Library/Logs/Hey Dot/llama-server.log`.
- Which local model runs is `pick_local_model(recommend(hardware))` until onboarding lets the user choose.

Called by: the user (launching the app) and CI (`.github/workflows/ci.yml`).
