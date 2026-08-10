# dot-runtime

Owns: one supervised llama-server process for a local model: picking its port, generating its API key, starting it, polling `/health` until it serves, restarting it after a crash, giving up after repeated crashes, and killing it on stop.

Must not know about: chat messages, providers, the UI, settings, the model catalog or where Hey Dot keeps its files. The caller passes the binary, model, projector and log paths.

Entry points: `Runtime::start(RuntimeConfig)`, then `watch_state()` for `Starting` / `Ready { base_url }` / `Down { reason }`, `api_key()` for the bearer token, and `stop().await`.

Invariants and gotchas:
- `Runtime::start` must run inside a tokio runtime; in the app that means `tauri::async_runtime`.
- The server listens on `127.0.0.1` only, on a fresh free port for every launch. llama-server binds its port only after the model loaded, so a port race shows up as a crash and the restart picks another port.
- The API key is 32 random bytes as hex, new for every `start`, and reaches llama-server through the `LLAMA_API_KEY` environment variable so it never shows in `ps`.
- `Ready` means `/health` answered 200. While the model loads llama-server refuses connections or answers 503.
- The health client skips every proxy: reqwest follows the macOS system proxy and `HTTP_PROXY` by default, and a proxy cannot reach this Mac's loopback. `tests/health_check_proxy.rs` pins it and is its own binary because it sets `HTTP_PROXY`.
- A crash restarts at once. The fourth exit within 60 seconds is `Down` with the last exit status, and nothing restarts after that until a new `start`.
- `stop` and dropping the `Runtime` both kill the server, including one still loading. A SIGKILL of the whole app leaves llama-server running as an orphan until it is killed by hand; it cannot answer anyone without the key.
- stdout and stderr are appended to `log_file`, whose folder must exist. Nothing rotates it yet.
- Tests run `fake-llama-server` (`tests/support/`), a std-only stand-in whose model file text picks ready, loading, crash or crash-first-launch. It is never shipped.
- `tests/real_model.rs` is ignored: it needs `scripts/build-llama-server.sh` run and Qwen3-VL 4B downloaded to `~/Library/Application Support/Hey Dot/models`. It is the proof that the pinned llama-server runs that GGUF.

Called by: `app/src-tauri` (`local_model.rs`).
