# Phase 0 + Phase 1 design: foundation and the core loop

Status: draft for review. Roadmap and stack decisions live in the approved plan; this spec covers only what Phase 0 and Phase 1 build.

## Outcome

At the end of Phase 1 a user downloads an (unsigned, beta) universal `.dmg`, opens Hey Dot, goes through onboarding, holds Option+Space, asks a question about what is on screen, sees the answer stream into a panel and hears it spoken, then asks a follow-up that uses the previous turn. Typing a question works the same way. The Python prototype is deleted.

Not in Phase 1: wake word, Ollama/LM Studio detection, Anthropic native API, model manager UI beyond first download, tools, MCP, persistent history, documents, Windows, signing.

## Phase 0: foundation

| Item | Detail |
|---|---|
| Safety point | Tag `v0-hackathon` on `58f4239`, work on branch `rebuild` (done) |
| License | `LICENSE` = Apache-2.0 full text. Contribution docs, CLA and any dual licensing are deferred to a later phase |
| Layout | `git mv frontend site`; `app/` Tauri 2 scaffold (React + TS + Vite); `crates/` Cargo workspace rooted at repo `Cargo.toml` including `app/src-tauri` |
| Docs | Root `AGENTS.md` routing file (<60 lines); README rewritten for the new product (legacy prototype mentioned as tag `v0-hackathon`) |
| CI | `.github/workflows/ci.yml` on PR + push to `rebuild`/`main`, macos-latest: `pnpm -C app build` (runs `tsc`, and must precede cargo because `generate_context!` reads `app/dist`), `pnpm -C app test` (`vitest run`), `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, `pnpm -C app tauri build --target universal-apple-darwin --bundles app,dmg` uploaded as an artifact |
| Toolchain pins | `rust-toolchain.toml` (stable, pinned minor), `app/package.json` `packageManager: pnpm@12.5.1` |

## Phase 1: modules

Each crate gets an `AGENTS.md` (owns, must not know about, entry points, invariants, callers). Crate names below are final; public items are indicative and get fixed in the implementation plan.

### dot-audio: mic capture to 16 kHz mono frames
- cpal default input device, resampled to 16 kHz mono `f32`, delivered as 512-sample frames (32 ms, the Silero frame size) over a bounded channel.
- Must not know about VAD, speech, or the UI.
- Invariant: dropping the stream handle stops the device; overflow drops oldest frames and counts them (exposed for diagnostics).

### dot-listen: push-to-talk utterance capture
- Phase 1 only: `record_utterance(frames, stop_signal)` collects frames between hotkey press and release. In "tap to talk" mode it stops on Silero VAD end of speech (800 ms trailing silence, configurable) or a 60 s hard cap.
- Silero VAD v6 ONNX via `ort`. Leading silence trimmed, 300 ms pre-roll kept so the first word is not clipped.
- Phase 2 adds the wake word here; nothing else changes.

### dot-stt: utterance audio to text
- Parakeet TDT 0.6B v3 (int8 ONNX) through sherpa-onnx's Rust bindings. First implementation step verifies the current crate and API; fallback is `transcribe-rs`.
- Model loaded once at startup in the background; `transcribe(&[f32]) -> Result<String>`.
- Invariant: never called on the audio thread.

### dot-screen: capture the screen the user is looking at
- ScreenCaptureKit `SCScreenshotManager` via the `screencapturekit` crate, display under the mouse cursor, excluding Hey Dot's own windows.
- Downscale so the long edge is at most 1600 px (setting), encode JPEG q85. The number is tuned with the screenshot QA eval, not guessed further.
- Returns a typed error for "permission not granted" so onboarding can explain it; never silently continues without an image.

### dot-models: which model to use and getting it on disk
- Compiled-in catalog: model id, display name, files (HuggingFace URL, sha256, bytes), min RAM, tier. Phase 1 entries: Qwen3-VL 2B, 4B, 8B GGUF Q4_K_M + mmproj; Parakeet v3; Silero VAD; Kokoro.
- `recommend(hardware)`: Apple Silicon 16 GB+ -> 4B, 32 GB+ offers 8B, Apple Silicon 8 GB -> 2B with quality note, Intel -> recommend cloud, local 2B allowed.
- Downloads to `~/Library/Application Support/Hey Dot/models/` with HTTP range resume, progress events, sha256 verify, atomic rename. A failed hash deletes the partial file and reports it.

### dot-runtime: keep llama-server running
- `llama-server` shipped as a Tauri `externalBin` (pinned llama.cpp release, universal binary built in CI from source).
- Starts with `-m`, `--mmproj`, `--jinja`, `-c 8192`, `--host 127.0.0.1`, a free port; polls `/health` until ready; restarts on crash up to 3 times in 60 s, then reports "runtime down".
- Must not know about chat messages; exposes only `base_url()` and a state stream.

### dot-providers: one streaming chat client
- Phase 1 has exactly one implementation: an OpenAI-compatible `/chat/completions` streaming client (SSE) with image parts. It serves llama-server and cloud providers that expose the OpenAI format (OpenAI, Gemini's OpenAI endpoint, OpenRouter). No trait until Anthropic native lands in Phase 3.
- `stream_chat(config, messages) -> Stream<Result<Delta>>`; errors mapped to: unreachable, auth failed, model not found, rate limited, other (with body).
- Each request carries a `leaves_device: bool` derived from the base URL, surfaced in the UI.

### dot-agent: one conversation turn at a time
- `Session` holds turns. `ask(UserInput { text, screenshot }) -> Stream<AgentEvent>` where events are `Thinking`, `Delta(text)`, `Done`, `Error(kind)`.
- Context policy: system prompt + last 10 turns; only the newest turn carries its image, older turns keep text with `[screenshot from earlier turn]`. Session resets after 10 min idle or on "New chat".
- One turn in flight; a new question cancels the running one (and its speech).

### dot-tts: speak streamed text, stop instantly
- Kokoro-82M ONNX; text deltas buffered and split at sentence boundaries; each sentence synthesized and queued to `rodio` playback so speech starts after the first sentence.
- `stop()` clears the queue and halts playback within one audio buffer (barge-in: hotkey press, Esc, or a new question).
- Fallback while Kokoro is downloading: macOS `AVSpeechSynthesizer` via objc2.
- **Risk to resolve first in the plan:** Kokoro's phonemizer commonly relies on espeak-ng (GPL-3). Linking it would put GPL obligations on the distributed Apache-2.0 app and block a later license change. The plan's first TTS step picks a non-GPL G2P (e.g. misaki-style lexicon + rules) or, if none is good enough, we decide together.

### dot-settings: settings file and secrets
- TOML at `~/Library/Application Support/Hey Dot/settings.toml`, typed struct with defaults, versioned for migration.
- API keys only in the macOS Keychain via `keyring`; never written to the TOML or logs.

### app: Tauri shell and UI
- Windows:
  - **Overlay pill**: non-activating NSPanel (tauri-nspanel), bottom center, states `idle`, `listening` (live mic level), `thinking`, `speaking`, `error`. Click opens the chat panel.
  - **Chat panel**: streamed markdown (react-markdown + code highlighting), text input, "New chat", per-answer badge "on this Mac" or "sent to <host>", copy button.
  - **Onboarding**: welcome, mic permission, screen permission (explains macOS' monthly re-confirm), model path (download recommended local model with progress, or paste a cloud key + base URL + model), test question.
  - **Settings**: hotkeys, talk mode (hold / tap), voice + speed, screenshot max size, model / provider, launch at login.
- Tray: open chat, pause listening, settings, quit.
- Hotkeys: hold Option+Space to talk (tap-to-talk if set), Option+Shift+Space toggle chat panel, Esc stop speaking.
- Rust -> UI events: `state`, `transcript`, `answer_delta`, `answer_done`, `download_progress`, `error`. UI -> Rust commands: `ask_text`, `new_chat`, `stop_speaking`, settings get/set, onboarding steps.
- Logs: `tracing` to a rotating file in `~/Library/Logs/Hey Dot/`; no prompt text or transcripts in logs.

## Error states the user sees

| Cause | What Dot shows / says |
|---|---|
| Mic permission denied | Pill error + panel card with "Open System Settings" button |
| Screen permission denied or expired | Answer text-only, with a card explaining and a button to re-grant |
| Model not downloaded / hash fail | Onboarding download step, retry button |
| llama-server down after retries | "Local model stopped", restart button, log path |
| Cloud: bad key / rate limit / offline | Specific message per error kind |
| STT heard nothing | Pill returns to idle with "Didn't catch that" |

## Testing

- Unit tests per crate. Fixtures in `crates/*/tests/fixtures`: WAV clips for VAD/STT (short known sentences), PNGs for downscale sizes, recorded SSE streams for provider parsing.
- `dot-providers` against a local mock server (`wiremock`): streaming, image parts, each error kind.
- `dot-models` download: range resume, hash mismatch, atomic rename, against a local file server.
- Model-dependent tests (`dot-stt`, `dot-runtime` real inference) marked `#[ignore]`, run locally and in a scheduled CI job with cached models.
- UI: vitest + Testing Library for pill states, chat streaming render, onboarding flow logic.
- `evals/latency`: scripted run printing hotkey release -> transcript, -> first token, -> first audio, per tier. `evals/screen-qa`: 30 screenshots with questions and expected answers, scored per downscale size and model. Numbers reported as measured, no targets invented before a baseline exists.

## Build order (one crate at a time, each its own commits)

1. Phase 0 files, restructure, scaffold, CI.
2. `dot-settings`, `dot-models` (pure logic, easy to test first).
3. `dot-runtime` + `dot-providers` -> typed question answered in a minimal chat panel (first end-to-end).
4. `dot-screen` -> screenshot attached to questions.
5. `dot-agent` session + follow-ups.
6. `dot-audio` + `dot-listen` + `dot-stt` -> push-to-talk.
7. `dot-tts` -> spoken answers with barge-in.
8. Overlay pill, onboarding, settings, tray, error cards.
9. Evals baseline, delete `hey_dot.py` and `requirements.txt`.

Steps 2, 4 and 6-7 are independent of each other and can run in parallel agents in separate worktrees once step 1 lands.
