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
