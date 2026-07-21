# dot-models

Owns: the compiled-in catalog of downloadable models (URLs pinned to upstream commits, sha256, sizes), the hardware-based recommendation of which chat model to use, and downloading a model's files into a models folder.

Must not know about: settings, the UI, Tauri, llama-server or any inference. It does not decide where the models folder is; the caller passes it (the app uses `~/Library/Application Support/Hey Dot/models/`).

Entry points: `CATALOG` and the per-model statics (`QWEN3_VL_4B`, `PARAKEET_TDT_V3`, ...); `detect_hardware()` then `recommend(&hardware)`; `download_model(&client, &model, models_dir, on_progress)`; `installed_chat_model(&model, models_dir)` for the weights and projector paths of a fully downloaded chat model.

Invariants and gotchas:
- A file exists at `<models_dir>/<model id>/<file name>` only after its sha256 matched and its bytes were synced to disk. Anything unverified is `<file name>.part`. "The final file exists" therefore means "installed"; nothing re-hashes installed files.
- A failed hash deletes the `.part` and returns `HashMismatch`; calling again starts that file from zero.
- Dropping the `download_model` future cancels it and keeps the `.part`; the next call resumes with a Range request. A server answering 200 instead of 206 restarts the file.
- One download per model at a time. Two concurrent calls on the same model write the same `.part`; the caller serialises them.
- `on_progress` fires per network chunk (many per second) and reaches 100% before the final hash, which takes a few seconds on multi-GB files. The caller throttles before forwarding to the UI.
- Every URL is pinned to a commit. Changing a file (new commit, other quant) means updating its sha256 and bytes in the same edit, then running `cargo test -p dot-models -- --ignored`, which checks every URL's size against the network.
- Kokoro is not in the catalog yet: its file set depends on the G2P decision in the dot-tts step.
- Qwen3-VL GGUF compatibility with the pinned llama-server is proven by `crates/dot-runtime/tests/real_model.rs` (ignored), not here.
- `installed_chat_model` tells weights from projector by the `mmproj-` file name prefix. A chat model must have exactly one of each.
- Licenses: Qwen3-VL Apache-2.0, Parakeet weights CC-BY-4.0 (attribution needed in the app's about screen), Silero VAD MIT.
- `recommend` thresholds are the chat models' `min_ram_gb` (4B: 16, 8B: 32), compared in whole GiB rounded down. Cloud is only returned for Intel; onboarding offers cloud to everyone anyway.
- The chip is the architecture the binary was compiled for. An Apple Silicon Mac running the app under Rosetta is seen as Intel.
- No free-disk-space check before downloading; a full disk surfaces as `DownloadError::Io`.

Called by: `app/src-tauri` (`local_model.rs`) and `crates/dot-runtime`'s ignored real-model test.
