# Phase 1 step 3: dot-runtime, dot-providers and the first chat panel Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** The first end-to-end path: a typed question in a minimal chat panel is answered by Qwen3-VL running in a bundled, supervised llama-server, streamed back token by token.

**Architecture:** Two new crates. `dot-runtime` supervises one llama-server process (free port, random API key via env, `/health` polling, restart up to 3 times in 60 s, kill on stop). `dot-providers` streams text deltas from any OpenAI-compatible `/chat/completions` endpoint and maps failures to the spec's five error kinds. The app gains `LocalModel` (download the picked model, start and watch its runtime, publish one status the UI renders) and three IPC commands; the React side gets a `ChatPanel` that shows the model's status and streams one answer. llama-server is built from a pinned llama.cpp tag by `scripts/build-llama-server.sh` and bundled as a Tauri `externalBin`.

**Tech Stack:** Rust 1.98 edition 2024, Tauri 2.11.6, `@tauri-apps/api` 2.11.1 (`Channel`, `mockIPC`); llama.cpp tag `b11123` (commit `384a534`), built with CMake for arm64 (Metal embedded) and x86_64, joined with `lipo`; `tokio` 1.53.1, `reqwest` 0.13.5, `url` 2.5.8, `getrandom` 0.4.3, `hex` 0.4.3, `eventsource-stream` 0.2.3, `async-stream` 0.3.6, `base64` 0.23.1, `futures-util` 0.3.34, `serde` 1.0.229, `serde_json` 1.0.151, `thiserror` 2.0.20; tests: `wiremock` 0.6.5, `tempfile` 3.27.0, vitest + Testing Library; CI: `actions/cache@v6.1.0`. Every file below was compile-checked and tested on 2026-09-23 in a scratch copy of the repo, including a real Qwen3-VL 4B answer and a bundled debug app that started and stopped llama-server.

**Spec:** `docs/specs/2026-09-23-phase0-phase1-design.md` (sections "dot-runtime", "dot-providers", "Chat panel", "Error states the user sees", "Testing", build order step 3).

## Global Constraints

- Crate names are final: `dot-runtime`, `dot-providers`. They join the workspace through the existing `members = ["app/src-tauri", "crates/*"]`; no root `Cargo.toml` edit.
- llama-server: "shipped as a Tauri `externalBin` (pinned llama.cpp release, universal binary built in CI from source)."
- llama-server flags: "`-m`, `--mmproj`, `--jinja`, `-c 8192`, `--host 127.0.0.1`, a free port; polls `/health` until ready; restarts on crash up to 3 times in 60 s, then reports "runtime down"."
- dot-runtime "Must not know about chat messages".
- dot-providers: "exactly one implementation: an OpenAI-compatible `/chat/completions` streaming client (SSE) with image parts ... No trait until Anthropic native lands in Phase 3."
- Errors "mapped to: unreachable, auth failed, model not found, rate limited, other (with body)."
- "Each request carries a `leaves_device: bool` derived from the base URL, surfaced in the UI." Badge text: "on this Mac" or "sent to <host>".
- Error state: "llama-server down after retries | "Local model stopped", restart button, log path".
- Tests: "`dot-providers` against a local mock server (`wiremock`): streaming, image parts, each error kind." "`dot-runtime` real inference marked `#[ignore]`, run locally."
- Every source file starts with a header saying why it exists and what it must not do; it never lists functions.
- Every crate has an `AGENTS.md`: owns, must not know about, entry points, invariants and gotchas, called by. "Called by" lines name callers added later in this plan; the plan lands as one push.
- Shell setup for every command: `export PATH="$HOME/.cargo/bin:$PATH"`, working directory the repo root.
- `pnpm -C app build` must have run before any workspace-wide cargo command. From Task 4 on, `scripts/build-llama-server.sh` must also have run before any cargo command that builds `hey-dot` (tauri-build rejects a missing `externalBin` or resource).
- Before Task 1: `git tag before-phase1-step3` (local only), the named point to return to.
- No em dashes in any file. Commits: one logical change each, no co-author lines, nothing pushed until the user has seen test output.

## Where this plan narrows the spec

- **The runtime's URL rides on its state.** The spec says dot-runtime "exposes only `base_url()` and a state stream". Here `RuntimeState::Ready { base_url }` carries the URL, so a reader can never see a URL from a previous launch after a restart moved the port. `api_key()` is the one extra accessor: llama-server is started with a fresh key so no other local process can use it.
- **Plain text, not markdown.** The panel shows the answer in a `pre-wrap` paragraph. react-markdown, code highlighting, "New chat" and the copy button land with the full chat panel (step 5, with dot-agent).
- **Local model only in the UI.** `ask_text` answers from the local model. `dot-providers` already speaks to cloud endpoints and `leaves_device` is wired end to end (the badge reads "Sent to <host>" in a test), but choosing a cloud provider comes with onboarding (step 8).
- **No system prompt, no history, no image.** One user message per question. dot-agent (step 5) owns context; dot-screen (step 4) brings the screenshot and the first real image inference.
- **No restart button, no startup timeout.** "Down" shows its reason and the log path; relaunching the app restarts the model. The button lands with the error-state work in step 8. A llama-server that never becomes healthy stays "Starting".
- **The model is picked, not chosen.** `pick_local_model` takes the first local model the hardware recommendation offers (Intel gets 2B behind the cloud recommendation) until onboarding (step 8) asks the user.
- **Licenses are bundled now, shown later.** llama.cpp, cpp-httplib and nlohmann/json license files go into `Resources/licenses/llama.cpp/`; the about screen that lists them is step 8.
- **No scheduled CI job for ignored tests yet.** `real_model.rs` runs locally. CI caches the built llama-server by the build script's hash.
- **Known leaks, accepted for this step:** a webview reload leaves the old status watcher task running until its channel send fails; React StrictMode in dev calls `watch_local_model` twice. Both cost one idle task. A SIGKILL of the whole app leaves llama-server orphaned (it cannot answer without the key). The llama-server log is never rotated.
- **The question box clears on Ask**, as chat apps do; the question stays visible above its answer.

## Review Focus

- llama-server dies while the user waits (crash during load or after ready): expect a restart on a new port and "ready" again, with no user action, and "stopped" plus the log path after the fourth crash in a minute. Task 3 tests `restarts_a_crashed_server_and_becomes_ready` and `gives_up_after_the_fourth_crash_within_a_minute`.
- llama-server is killed mid-answer: expect the text so far to stay on screen with an error under it, never a silently short answer. Task 2 test `stream_that_ends_without_done_is_unreachable_after_the_text_so_far`, Task 6 test "a failed answer keeps the text so far and shows the error".
- The user quits while the model is still loading: expect llama-server to die with the app. Task 3 test `stop_kills_a_server_that_is_still_loading`, Task 7 manual quit check with `pgrep -x llama-server`.
- The user double-clicks Download: expect one download, not two writers on the same `.part` file. Task 5 test `a_second_download_while_one_runs_returns_at_once`.
- A question arrives before the model is ready: expect a disabled Ask button, and a clear refusal if a command still arrives. Task 6 test "ask stays disabled until the model is ready", Task 5 test `a_model_that_is_not_running_has_no_chat_config`.

---

### Task 1: dot-models tells whether a chat model is installed

**Files:**
- Create: `crates/dot-models/src/installed.rs`
- Create: `crates/dot-models/tests/installed.rs`
- Modify: `crates/dot-models/src/lib.rs` (module and export)
- Modify: `crates/dot-models/src/download.rs:15,49` (use the shared `model_dir`)
- Modify: `crates/dot-models/AGENTS.md`

**Interfaces:**
- Consumes: `dot_models::{Model, ModelFile, QWEN3_VL_4B, PARAKEET_TDT_V3}` from step 2.
- Produces: `dot_models::{ChatModelFiles { weights: PathBuf, projector: PathBuf }, installed_chat_model(&Model, &Path) -> Option<ChatModelFiles>}`. Used by Task 5 (`LocalModel`) and Task 7 (`real_model.rs`).

- [ ] **Step 1: Write the failing tests**

`crates/dot-models/tests/installed.rs`:

```rust
use std::fs;
use std::path::Path;

use dot_models::{ChatModelFiles, QWEN3_VL_4B, SILERO_VAD_V6, installed_chat_model};
use tempfile::TempDir;

const WEIGHTS: &str = "Qwen3VL-4B-Instruct-Q4_K_M.gguf";
const PROJECTOR: &str = "mmproj-Qwen3VL-4B-Instruct-F16.gguf";

fn put_file(models_dir: &Path, model_id: &str, file_name: &str) {
    let model_dir = models_dir.join(model_id);
    fs::create_dir_all(&model_dir).unwrap();
    fs::write(model_dir.join(file_name), b"x").unwrap();
}

#[test]
fn installed_chat_model_returns_weights_and_projector_paths() {
    let models_dir = TempDir::new().unwrap();
    put_file(models_dir.path(), QWEN3_VL_4B.id, WEIGHTS);
    put_file(models_dir.path(), QWEN3_VL_4B.id, PROJECTOR);

    let model_dir = models_dir.path().join(QWEN3_VL_4B.id);
    assert_eq!(
        installed_chat_model(&QWEN3_VL_4B, models_dir.path()),
        Some(ChatModelFiles {
            weights: model_dir.join(WEIGHTS),
            projector: model_dir.join(PROJECTOR),
        })
    );
}

#[test]
fn nothing_downloaded_is_not_installed() {
    let models_dir = TempDir::new().unwrap();

    assert_eq!(installed_chat_model(&QWEN3_VL_4B, models_dir.path()), None);
}

#[test]
fn a_projector_still_partial_is_not_installed() {
    let models_dir = TempDir::new().unwrap();
    put_file(models_dir.path(), QWEN3_VL_4B.id, WEIGHTS);
    put_file(
        models_dir.path(),
        QWEN3_VL_4B.id,
        &format!("{PROJECTOR}.part"),
    );

    assert_eq!(installed_chat_model(&QWEN3_VL_4B, models_dir.path()), None);
}

#[test]
fn a_model_without_a_projector_is_not_a_chat_model() {
    let models_dir = TempDir::new().unwrap();
    for file in SILERO_VAD_V6.files {
        put_file(models_dir.path(), SILERO_VAD_V6.id, file.name);
    }

    assert_eq!(
        installed_chat_model(&SILERO_VAD_V6, models_dir.path()),
        None
    );
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p dot-models --test installed`
Expected: compile error `unresolved import` for `dot_models::installed_chat_model`.

- [ ] **Step 3: Implement `installed_chat_model`**

`crates/dot-models/src/installed.rs`:

```rust
//! Says where an installed chat model's files are.
//! Must not download, hash or delete anything: a file under its final name is already verified.

use std::path::{Path, PathBuf};

use crate::catalog::Model;

const PROJECTOR_FILE_PREFIX: &str = "mmproj-";

/// The two files llama-server loads for a vision chat model.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatModelFiles {
    pub weights: PathBuf,
    pub projector: PathBuf,
}

pub(crate) fn model_dir(model: &Model, models_dir: &Path) -> PathBuf {
    models_dir.join(model.id)
}

/// `None` while any file of `model` is missing, and for models without exactly one weights
/// file and one `mmproj-` projector file (speech and VAD models).
pub fn installed_chat_model(model: &Model, models_dir: &Path) -> Option<ChatModelFiles> {
    let dir = model_dir(model, models_dir);
    if !model.files.iter().all(|file| dir.join(file.name).is_file()) {
        return None;
    }
    let (projectors, weights): (Vec<_>, Vec<_>) = model
        .files
        .iter()
        .partition(|file| file.name.starts_with(PROJECTOR_FILE_PREFIX));
    match (weights.as_slice(), projectors.as_slice()) {
        ([weights], [projector]) => Some(ChatModelFiles {
            weights: dir.join(weights.name),
            projector: dir.join(projector.name),
        }),
        _ => None,
    }
}
```

In `crates/dot-models/src/lib.rs` add `mod installed;` after `mod hardware;` and add this export after the `hardware` export:

```rust
pub use installed::{ChatModelFiles, installed_chat_model};
```

In `crates/dot-models/src/download.rs` add `use crate::installed::model_dir;` after `use crate::catalog::{Model, ModelFile};`, and replace

```rust
    let model_dir = models_dir.join(model.id);
```

with

```rust
    let model_dir = model_dir(model, models_dir);
```

so the download and the installed check can never disagree on the folder.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p dot-models`
Expected: `installed` target `4 passed`; `download` `8 passed; 1 ignored`, `catalog` `4 passed; 1 ignored`, `recommend` `6 passed`.

- [ ] **Step 5: Update the module doc**

In `crates/dot-models/AGENTS.md` replace the "Entry points" line with:

```markdown
Entry points: `CATALOG` and the per-model statics (`QWEN3_VL_4B`, `PARAKEET_TDT_V3`, ...); `detect_hardware()` then `recommend(&hardware)`; `download_model(&client, &model, models_dir, on_progress)`; `installed_chat_model(&model, models_dir)` for the weights and projector paths of a fully downloaded chat model.
```

and add this bullet after the "Qwen3-VL GGUF compatibility" bullet:

```markdown
- `installed_chat_model` tells weights from projector by the `mmproj-` file name prefix. A chat model must have exactly one of each.
```

- [ ] **Step 6: Format, lint, commit**

```bash
cargo fmt --all
cargo clippy -p dot-models --all-targets -- -D warnings
cargo test -p dot-models
git add crates/dot-models
git commit -m "Tell whether a chat model's weights and projector are installed"
```

Expected: no clippy warnings; all dot-models targets pass as in Step 4.

---

### Task 2: dot-providers streams a chat answer

**Files:**
- Create: `crates/dot-providers/Cargo.toml`
- Create: `crates/dot-providers/src/lib.rs`
- Create: `crates/dot-providers/src/chat_request.rs`
- Create: `crates/dot-providers/src/provider_error.rs`
- Create: `crates/dot-providers/src/stream_chat.rs`
- Create: `crates/dot-providers/tests/stream_chat.rs`
- Create: `crates/dot-providers/tests/leaves_device.rs`
- Create: `crates/dot-providers/AGENTS.md`
- Modify: `AGENTS.md` (one module line)
- Modify: `Cargo.lock`

**Interfaces:**
- Consumes: nothing.
- Produces: `dot_providers::{ChatConfig { base_url: Url, api_key: String, model: String }, ChatConfig::leaves_device(&self) -> bool, ChatMessage { role: ChatRole, text: String, jpeg_image: Option<Vec<u8>> }, ChatRole::{System, User, Assistant}, ProviderError::{Unreachable(String), AuthFailed, ModelNotFound, RateLimited, Other { status: Option<u16>, body: String }}, stream_chat(&reqwest::Client, &ChatConfig, &[ChatMessage]) -> impl Stream<Item = Result<String, ProviderError>>}`. `ChatConfig` derives `Debug, Clone, PartialEq, Eq`. Used by Task 5 (`commands.rs`, `local_model.rs`) and Task 7.

- [ ] **Step 1: Create the crate**

`crates/dot-providers/Cargo.toml`:

```toml
[package]
name = "dot-providers"
version = "0.1.0"
edition = "2024"
license = "Apache-2.0"
publish = false

[dependencies]
async-stream = "0.3.6"
base64 = "0.23.1"
eventsource-stream = "0.2.3"
futures-util = "0.3.34"
reqwest = { version = "0.13.5", features = ["json", "stream"] }
serde = { version = "1.0.229", features = ["derive"] }
serde_json = "1.0.151"
thiserror = "2.0.20"
url = "2.5.8"

[dev-dependencies]
tokio = { version = "1.53.1", features = ["macros", "rt-multi-thread"] }
wiremock = "0.6.5"
serde_json = "1.0.151"
```

`crates/dot-providers/src/lib.rs`, header only for now:

```rust
//! Streams a chat answer from any OpenAI-compatible `/chat/completions` endpoint.
//! Must not start servers, pick models, hold conversation history or know which provider is behind the URL.
```

Run: `cargo check -p dot-providers`
Expected: `Finished`; `Cargo.lock` gains the new dependencies.

- [ ] **Step 2: Write the failing tests**

`crates/dot-providers/tests/stream_chat.rs`:

```rust
use dot_providers::{ChatConfig, ChatMessage, ChatRole, ProviderError, stream_chat};
use futures_util::StreamExt;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn config_for(server: &MockServer) -> ChatConfig {
    ChatConfig {
        base_url: format!("{}/v1", server.uri()).parse().unwrap(),
        api_key: "test-key".to_string(),
        model: "test-model".to_string(),
    }
}

fn question(text: &str) -> Vec<ChatMessage> {
    vec![ChatMessage {
        role: ChatRole::User,
        text: text.to_string(),
        jpeg_image: None,
    }]
}

async fn server_answering(response: ResponseTemplate) -> MockServer {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(response)
        .mount(&server)
        .await;
    server
}

fn sse(events: &[&str]) -> ResponseTemplate {
    let body: String = events
        .iter()
        .map(|event| format!("data: {event}\n\n"))
        .collect();
    ResponseTemplate::new(200).set_body_raw(body, "text/event-stream")
}

async fn collect_answer(
    config: &ChatConfig,
    messages: &[ChatMessage],
) -> Vec<Result<String, ProviderError>> {
    let client = reqwest::Client::new();
    stream_chat(&client, config, messages).collect().await
}

async fn first_error(server: &MockServer) -> ProviderError {
    let items = collect_answer(&config_for(server), &question("hi")).await;
    items
        .into_iter()
        .find_map(Result::err)
        .expect("expected an error")
}

#[tokio::test]
async fn yields_text_deltas_in_order_and_skips_chunks_without_text() {
    let server = server_answering(sse(&[
        r#"{"choices":[{"delta":{"role":"assistant","content":null}}]}"#,
        r#"{"choices":[{"delta":{"content":"Par"}}]}"#,
        r#"{"choices":[{"delta":{"content":""}}]}"#,
        r#"{"choices":[{"delta":{"content":"is"}}]}"#,
        r#"{"choices":[{"delta":{},"finish_reason":"stop"}]}"#,
        r#"{"choices":[],"usage":{"completion_tokens":2}}"#,
        "[DONE]",
    ]))
    .await;

    let items = collect_answer(&config_for(&server), &question("capital of France?")).await;

    assert_eq!(items, vec![Ok("Par".to_string()), Ok("is".to_string())]);
}

#[tokio::test]
async fn posts_model_bearer_key_and_stream_flag_to_chat_completions_under_the_base_url() {
    let server = server_answering(sse(&["[DONE]"])).await;

    collect_answer(&config_for(&server), &question("hi")).await;

    let requests = server.received_requests().await.unwrap();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].url.path(), "/v1/chat/completions");
    assert_eq!(requests[0].headers["authorization"], "Bearer test-key");
    let body: serde_json::Value = requests[0].body_json().unwrap();
    assert_eq!(
        body,
        serde_json::json!({
            "model": "test-model",
            "stream": true,
            "messages": [{"role": "user", "content": [{"type": "text", "text": "hi"}]}]
        })
    );
}

#[tokio::test]
async fn sends_an_image_as_a_jpeg_data_uri_after_the_text() {
    let server = server_answering(sse(&["[DONE]"])).await;
    let messages = vec![ChatMessage {
        role: ChatRole::User,
        text: "what is this?".to_string(),
        jpeg_image: Some(vec![0xFF, 0xD8, 0xFF]),
    }];

    collect_answer(&config_for(&server), &messages).await;

    let body: serde_json::Value = server.received_requests().await.unwrap()[0]
        .body_json()
        .unwrap();
    assert_eq!(
        body["messages"][0]["content"],
        serde_json::json!([
            {"type": "text", "text": "what is this?"},
            {"type": "image_url", "image_url": {"url": "data:image/jpeg;base64,/9j/"}}
        ])
    );
}

#[tokio::test]
async fn sends_system_and_assistant_roles_in_lowercase() {
    let server = server_answering(sse(&["[DONE]"])).await;
    let messages = vec![
        ChatMessage {
            role: ChatRole::System,
            text: "be brief".to_string(),
            jpeg_image: None,
        },
        ChatMessage {
            role: ChatRole::Assistant,
            text: "ok".to_string(),
            jpeg_image: None,
        },
    ];

    collect_answer(&config_for(&server), &messages).await;

    let body: serde_json::Value = server.received_requests().await.unwrap()[0]
        .body_json()
        .unwrap();
    assert_eq!(body["messages"][0]["role"], "system");
    assert_eq!(body["messages"][1]["role"], "assistant");
}

#[tokio::test]
async fn status_401_and_403_are_auth_failed() {
    for status in [401, 403] {
        let server = server_answering(ResponseTemplate::new(status)).await;
        assert_eq!(
            first_error(&server).await,
            ProviderError::AuthFailed,
            "status {status}"
        );
    }
}

#[tokio::test]
async fn gemini_bad_key_400_is_auth_failed() {
    let body = r#"[{"error":{"code":400,"message":"API key not valid. Please pass a valid API key.","status":"INVALID_ARGUMENT"}}]"#;
    let server = server_answering(ResponseTemplate::new(400).set_body_string(body)).await;

    assert_eq!(first_error(&server).await, ProviderError::AuthFailed);
}

#[tokio::test]
async fn other_400_keeps_status_and_body() {
    let server =
        server_answering(ResponseTemplate::new(400).set_body_string("context too long")).await;

    assert_eq!(
        first_error(&server).await,
        ProviderError::Other {
            status: Some(400),
            body: "context too long".to_string()
        }
    );
}

#[tokio::test]
async fn status_404_is_model_not_found() {
    let server = server_answering(ResponseTemplate::new(404)).await;

    assert_eq!(first_error(&server).await, ProviderError::ModelNotFound);
}

#[tokio::test]
async fn status_429_is_rate_limited() {
    let server = server_answering(ResponseTemplate::new(429)).await;

    assert_eq!(first_error(&server).await, ProviderError::RateLimited);
}

#[tokio::test]
async fn status_500_keeps_status_and_body() {
    let server = server_answering(ResponseTemplate::new(500).set_body_string("boom")).await;

    assert_eq!(
        first_error(&server).await,
        ProviderError::Other {
            status: Some(500),
            body: "boom".to_string()
        }
    );
}

#[tokio::test]
async fn closed_port_is_unreachable() {
    // wiremock pools its servers, so a dropped MockServer keeps listening; use a port nobody holds.
    let free_port = std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    let config = ChatConfig {
        base_url: format!("http://127.0.0.1:{free_port}/v1").parse().unwrap(),
        api_key: "test-key".to_string(),
        model: "test-model".to_string(),
    };

    let items = collect_answer(&config, &question("hi")).await;

    assert!(
        matches!(items.as_slice(), [Err(ProviderError::Unreachable(_))]),
        "got {items:?}"
    );
}

#[tokio::test]
async fn error_chunk_mid_stream_ends_the_answer_with_its_body() {
    let server = server_answering(sse(&[
        r#"{"choices":[{"delta":{"content":"Par"}}]}"#,
        r#"{"error":{"code":500,"message":"slot unavailable"}}"#,
    ]))
    .await;

    let items = collect_answer(&config_for(&server), &question("hi")).await;

    assert_eq!(items.len(), 2);
    assert_eq!(items[0], Ok("Par".to_string()));
    assert!(
        matches!(&items[1], Err(ProviderError::Other { status: None, body }) if body.contains("slot unavailable")),
        "got {:?}",
        items[1]
    );
}

#[tokio::test]
async fn stream_that_ends_without_done_is_unreachable_after_the_text_so_far() {
    let server = server_answering(sse(&[r#"{"choices":[{"delta":{"content":"Par"}}]}"#])).await;

    let items = collect_answer(&config_for(&server), &question("hi")).await;

    assert_eq!(items.len(), 2);
    assert_eq!(items[0], Ok("Par".to_string()));
    assert!(
        matches!(items[1], Err(ProviderError::Unreachable(_))),
        "got {:?}",
        items[1]
    );
}

#[tokio::test]
async fn unparsable_chunk_is_an_error_with_the_raw_data() {
    let server = server_answering(sse(&["not json", "[DONE]"])).await;

    assert_eq!(
        first_error(&server).await,
        ProviderError::Other {
            status: None,
            body: "not json".to_string()
        }
    );
}
```

`crates/dot-providers/tests/leaves_device.rs`:

```rust
use dot_providers::ChatConfig;

fn config_with(base_url: &str) -> ChatConfig {
    ChatConfig {
        base_url: base_url.parse().unwrap(),
        api_key: String::new(),
        model: String::new(),
    }
}

#[test]
fn loopback_addresses_stay_on_this_mac() {
    for base_url in [
        "http://127.0.0.1:8080/v1",
        "http://localhost:11434/v1",
        "http://[::1]:1234/v1",
    ] {
        assert!(!config_with(base_url).leaves_device(), "{base_url}");
    }
}

#[test]
fn other_hosts_leave_this_mac() {
    for base_url in [
        "https://api.openai.com/v1",
        "http://192.168.1.20:8080/v1",
        "http://localhost.example.com/v1",
    ] {
        assert!(config_with(base_url).leaves_device(), "{base_url}");
    }
}
```

- [ ] **Step 3: Run the tests to verify they fail**

Run: `cargo test -p dot-providers`
Expected: compile errors `unresolved imports` for `dot_providers::ChatConfig`, `ChatMessage`, `ChatRole`, `ProviderError`, `stream_chat`.

- [ ] **Step 4: Implement the request, the errors and the stream**

`crates/dot-providers/src/lib.rs`:

```rust
//! Streams a chat answer from any OpenAI-compatible `/chat/completions` endpoint.
//! Must not start servers, pick models, hold conversation history or know which provider is behind the URL.

mod chat_request;
mod provider_error;
mod stream_chat;

pub use chat_request::{ChatConfig, ChatMessage, ChatRole};
pub use provider_error::ProviderError;
pub use stream_chat::stream_chat;
```

`crates/dot-providers/src/chat_request.rs`:

```rust
//! What a chat request contains and its OpenAI wire format.
//! Must not send anything; `stream_chat` owns the HTTP call.

use base64::Engine;
use serde::Serialize;
use url::{Host, Url};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatConfig {
    /// The OpenAI-compatible root, ending in `/v1` for llama-server and OpenAI.
    pub base_url: Url,
    pub api_key: String,
    pub model: String,
}

impl ChatConfig {
    /// True when the request goes to another machine. Only loopback addresses stay on this Mac.
    pub fn leaves_device(&self) -> bool {
        match self.base_url.host() {
            Some(Host::Domain(domain)) => domain != "localhost",
            Some(Host::Ipv4(address)) => !address.is_loopback(),
            Some(Host::Ipv6(address)) => !address.is_loopback(),
            None => true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ChatRole {
    System,
    User,
    Assistant,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatMessage {
    pub role: ChatRole,
    pub text: String,
    pub jpeg_image: Option<Vec<u8>>,
}

#[derive(Serialize)]
pub(crate) struct RequestBody<'a> {
    model: &'a str,
    messages: Vec<RequestMessage<'a>>,
    stream: bool,
}

#[derive(Serialize)]
struct RequestMessage<'a> {
    role: ChatRole,
    content: Vec<ContentPart<'a>>,
}

#[derive(Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum ContentPart<'a> {
    Text { text: &'a str },
    ImageUrl { image_url: ImageUrl },
}

#[derive(Serialize)]
struct ImageUrl {
    url: String,
}

pub(crate) fn build_request_body<'a>(
    config: &'a ChatConfig,
    messages: &'a [ChatMessage],
) -> RequestBody<'a> {
    let messages = messages
        .iter()
        .map(|message| {
            let mut content = vec![ContentPart::Text {
                text: &message.text,
            }];
            if let Some(jpeg) = &message.jpeg_image {
                let encoded = base64::engine::general_purpose::STANDARD.encode(jpeg);
                content.push(ContentPart::ImageUrl {
                    image_url: ImageUrl {
                        url: format!("data:image/jpeg;base64,{encoded}"),
                    },
                });
            }
            RequestMessage {
                role: message.role,
                content,
            }
        })
        .collect();
    RequestBody {
        model: &config.model,
        messages,
        stream: true,
    }
}
```

`crates/dot-providers/src/provider_error.rs`:

```rust
//! The error kinds the UI tells apart, and how HTTP failures map onto them.
//! Must not decide what the user is shown; the caller words the message.

use reqwest::StatusCode;
use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ProviderError {
    #[error("could not reach the model server: {0}")]
    Unreachable(String),
    #[error("the API key was rejected")]
    AuthFailed,
    #[error("the model was not found")]
    ModelNotFound,
    #[error("the provider is rate limiting requests")]
    RateLimited,
    #[error("the model server returned an error{}: {body}", status.map(|code| format!(" (HTTP {code})")).unwrap_or_default())]
    Other { status: Option<u16>, body: String },
}

pub(crate) fn error_for_status(status: StatusCode, body: String) -> ProviderError {
    match status {
        StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => ProviderError::AuthFailed,
        // Gemini's OpenAI endpoint answers a bad key with 400 instead of 401.
        StatusCode::BAD_REQUEST if body.contains("API key not valid") => ProviderError::AuthFailed,
        StatusCode::NOT_FOUND => ProviderError::ModelNotFound,
        StatusCode::TOO_MANY_REQUESTS => ProviderError::RateLimited,
        _ => ProviderError::Other {
            status: Some(status.as_u16()),
            body,
        },
    }
}

pub(crate) fn error_for_transport(error: reqwest::Error) -> ProviderError {
    ProviderError::Unreachable(error.to_string())
}
```

`crates/dot-providers/src/stream_chat.rs`:

```rust
//! Sends one chat request and yields the answer text as it streams in.
//! Must not retry, buffer the whole answer or log message content.

use eventsource_stream::{EventStreamError, Eventsource};
use futures_util::{Stream, StreamExt};
use serde::Deserialize;

use crate::chat_request::{ChatConfig, ChatMessage, build_request_body};
use crate::provider_error::{ProviderError, error_for_status, error_for_transport};

#[derive(Deserialize)]
struct StreamChunk {
    #[serde(default)]
    choices: Vec<StreamChoice>,
    error: Option<serde_json::Value>,
}

#[derive(Deserialize)]
struct StreamChoice {
    #[serde(default)]
    delta: StreamDelta,
}

#[derive(Deserialize, Default)]
struct StreamDelta {
    content: Option<String>,
}

/// Yields non-empty text deltas in order. The stream ends after `[DONE]`; a connection that
/// closes before `[DONE]` ends with `Unreachable`, so a cut-off answer never looks complete.
pub fn stream_chat<'a>(
    client: &'a reqwest::Client,
    config: &'a ChatConfig,
    messages: &'a [ChatMessage],
) -> impl Stream<Item = Result<String, ProviderError>> + 'a {
    async_stream::try_stream! {
        let url = format!("{}/chat/completions", config.base_url.as_str().trim_end_matches('/'));
        let response = client
            .post(url)
            .bearer_auth(&config.api_key)
            .json(&build_request_body(config, messages))
            .send()
            .await
            .map_err(error_for_transport)?;

        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            Err(error_for_status(status, body))?;
            return;
        }

        let mut events = response.bytes_stream().eventsource();
        let mut finished = false;
        while let Some(event) = events.next().await {
            let event = event.map_err(|error| match error {
                EventStreamError::Transport(error) => error_for_transport(error),
                other => ProviderError::Other { status: None, body: other.to_string() },
            })?;
            if event.data == "[DONE]" {
                finished = true;
                break;
            }
            let chunk: StreamChunk = serde_json::from_str(&event.data)
                .map_err(|_| ProviderError::Other { status: None, body: event.data.clone() })?;
            if let Some(error) = chunk.error {
                Err(ProviderError::Other { status: None, body: error.to_string() })?;
            }
            let text = chunk.choices.into_iter().next().and_then(|choice| choice.delta.content);
            if let Some(text) = text.filter(|text| !text.is_empty()) {
                yield text;
            }
        }
        if !finished {
            Err(ProviderError::Unreachable("the connection closed before the answer finished".to_string()))?;
        }
    }
}
```

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test -p dot-providers`
Expected: `leaves_device` `2 passed`, `stream_chat` `14 passed`.

- [ ] **Step 6: Write the module doc and route to it**

`crates/dot-providers/AGENTS.md`:

```markdown
# dot-providers

Owns: sending one chat request to an OpenAI-compatible `/chat/completions` endpoint and streaming the answer's text back, with the request's image as a JPEG data URI, and turning HTTP and stream failures into `ProviderError`. Also says whether a base URL leaves this Mac (`ChatConfig::leaves_device`).

Must not know about: llama-server, which provider or model sits behind the URL, conversation history, system prompts, settings or the UI. It does not start servers or retry.

Entry points: `stream_chat(&client, &config, &messages)`, a stream of text deltas; `ChatConfig::leaves_device()`.

Invariants and gotchas:
- `base_url` is the OpenAI-compatible root ending in `/v1`; the crate appends `/chat/completions`.
- Every request is `stream: true`. Chunks without text (role-only first chunk, finish chunk, usage-only chunk) yield nothing.
- An answer is complete only when `[DONE]` arrived. A stream that ends without it yields its text so far, then `Unreachable`, so a killed server never looks like a short answer.
- A chunk carrying `error` ends the stream with `Other` holding that error's JSON. A chunk that does not parse ends it with `Other` holding the raw data.
- Gemini's OpenAI endpoint answers a bad key with 400 "API key not valid" instead of 401; that maps to `AuthFailed` like 401 and 403.
- `leaves_device` is false only for `localhost`, `127.0.0.0/8` and `::1`. A LAN address such as `192.168.1.20` leaves the Mac.
- The caller owns the `reqwest::Client` and its timeouts. With no read timeout a stalled server stalls the stream forever.
- Tests use wiremock, which pools its servers: a dropped `MockServer` keeps listening and may answer the next test. "Unreachable" tests use a port freed from a `TcpListener`.

Called by: `app/src-tauri` (the `ask_text` command) and `crates/dot-runtime`'s ignored real-model test.
```

In the root `AGENTS.md`, after the `crates/dot-models/` line, add:

```markdown
- `crates/dot-providers/`: streaming client for OpenAI-compatible chat endpoints. See `crates/dot-providers/AGENTS.md`.
```

- [ ] **Step 7: Format, lint, commit**

```bash
cargo fmt --all
cargo clippy -p dot-providers --all-targets -- -D warnings
cargo test -p dot-providers
git add crates/dot-providers AGENTS.md Cargo.lock
git commit -m "Add dot-providers: stream answers from OpenAI-compatible chat endpoints"
```

Expected: no clippy warnings; `16 passed` across both targets.

---

### Task 3: dot-runtime supervises llama-server

**Files:**
- Create: `crates/dot-runtime/Cargo.toml`
- Create: `crates/dot-runtime/src/lib.rs`
- Create: `crates/dot-runtime/src/server_process.rs`
- Create: `crates/dot-runtime/src/runtime.rs`
- Create: `crates/dot-runtime/tests/support/fake_llama_server.rs`
- Create: `crates/dot-runtime/tests/runtime.rs`
- Create: `crates/dot-runtime/AGENTS.md`
- Modify: `AGENTS.md` (one module line)
- Modify: `Cargo.lock`

**Interfaces:**
- Consumes: nothing (tests run a fake server binary built from this crate).
- Produces: `dot_runtime::{RuntimeConfig { server_binary, model_file, mmproj_file: PathBuf, context_tokens: u32, log_file: PathBuf }, RuntimeState::{Starting, Ready { base_url: Url }, Down { reason: String }}, Runtime::start(RuntimeConfig) -> Runtime, Runtime::api_key(&self) -> &str, Runtime::watch_state(&self) -> tokio::sync::watch::Receiver<RuntimeState>, Runtime::stop(self) async}`. Used by Task 5 (`local_model.rs`) and Task 7.

- [ ] **Step 1: Create the crate with the fake server**

The tests never run the real llama-server: `fake-llama-server` is a std-only binary in this crate that records its arguments and `LLAMA_API_KEY`, then behaves as its model file's text says (`ready`, `loading`, `crash`, `crash-first-launch`). Cargo builds it for integration tests and exposes its path as `CARGO_BIN_EXE_fake-llama-server`.

`crates/dot-runtime/Cargo.toml` (the real-model test in Task 7 adds three dev-dependencies):

```toml
[package]
name = "dot-runtime"
version = "0.1.0"
edition = "2024"
license = "Apache-2.0"
publish = false

[[bin]]
name = "fake-llama-server"
path = "tests/support/fake_llama_server.rs"
test = false
doc = false

[dependencies]
getrandom = "0.4.3"
hex = "0.4.3"
reqwest = "0.13.5"
tokio = { version = "1.53.1", features = ["macros", "process", "rt", "sync", "time"] }
url = "2.5.8"

[dev-dependencies]
tempfile = "3.27.0"
tokio = { version = "1.53.1", features = ["macros", "rt-multi-thread"] }
```

`crates/dot-runtime/tests/support/fake_llama_server.rs`:

```rust
//! Stands in for llama-server in dot-runtime's tests; never shipped or run by the app.
//! The model file's text picks the behaviour: `ready`, `loading`, `crash` or `crash-first-launch`.
//! Next to the model file it records the arguments, the API key and one line per launch.

use std::fs::{self, OpenOptions};
use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::PathBuf;

const HEALTH_CHECKS_BEFORE_READY: usize = 2;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let value_after = |flag: &str| {
        let position = args
            .iter()
            .position(|arg| arg == flag)
            .expect("flag missing");
        args[position + 1].clone()
    };
    let model_file = PathBuf::from(value_after("-m"));
    let port = value_after("--port");
    let recording = |extension: &str| model_file.with_extension(extension);

    fs::write(recording("args"), args.join("\n")).unwrap();
    fs::write(
        recording("api_key"),
        std::env::var("LLAMA_API_KEY").unwrap_or_default(),
    )
    .unwrap();
    let mut launches = OpenOptions::new()
        .create(true)
        .append(true)
        .open(recording("launches"))
        .unwrap();
    writeln!(launches, "launch").unwrap();
    let launch_count = fs::read_to_string(recording("launches"))
        .unwrap()
        .lines()
        .count();

    let mode = fs::read_to_string(&model_file).unwrap();
    let health_checks_before_ready = match (mode.trim(), launch_count) {
        ("crash", _) | ("crash-first-launch", 1) => std::process::exit(1),
        ("loading", _) => usize::MAX,
        _ => HEALTH_CHECKS_BEFORE_READY,
    };

    let listener = TcpListener::bind(format!("127.0.0.1:{port}")).unwrap();
    for (served, connection) in listener.incoming().enumerate() {
        let mut connection = connection.unwrap();
        let mut request = [0u8; 4096];
        let _ = connection.read(&mut request);
        let status_line = if served < health_checks_before_ready {
            "503 Service Unavailable"
        } else {
            "200 OK"
        };
        let body = r#"{"status":"ok"}"#;
        let _ = write!(
            connection,
            "HTTP/1.1 {status_line}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
    }
}
```

`crates/dot-runtime/src/lib.rs`, header only for now:

```rust
//! Keeps one llama-server process running for a local model and says whether it can take requests.
//! Must not know about chat messages, providers, the UI or where Hey Dot keeps its files.
```

Run: `cargo build -p dot-runtime --bin fake-llama-server`
Expected: `Finished`.

- [ ] **Step 2: Write the failing tests**

`crates/dot-runtime/tests/runtime.rs`:

```rust
use std::fs;
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::time::Duration;

use dot_runtime::{Runtime, RuntimeConfig, RuntimeState};
use tempfile::TempDir;

const WAIT_LIMIT: Duration = Duration::from_secs(10);

fn config_for(folder: &Path, fake_mode: &str) -> RuntimeConfig {
    let model_file = folder.join("model.gguf");
    fs::write(&model_file, fake_mode).unwrap();
    RuntimeConfig {
        server_binary: PathBuf::from(env!("CARGO_BIN_EXE_fake-llama-server")),
        model_file,
        mmproj_file: folder.join("mmproj.gguf"),
        context_tokens: 8192,
        log_file: folder.join("llama-server.log"),
    }
}

async fn wait_for_state(runtime: &Runtime, wanted: impl Fn(&RuntimeState) -> bool) -> RuntimeState {
    let mut states = runtime.watch_state();
    let state = tokio::time::timeout(WAIT_LIMIT, states.wait_for(|state| wanted(state)))
        .await
        .expect("timed out waiting for the runtime state")
        .expect("the supervisor stopped before reaching the state");
    state.clone()
}

async fn wait_until_ready(runtime: &Runtime) -> u16 {
    match wait_for_state(runtime, |state| matches!(state, RuntimeState::Ready { .. })).await {
        RuntimeState::Ready { base_url } => base_url.port().unwrap(),
        _ => unreachable!(),
    }
}

async fn wait_until_down(runtime: &Runtime) -> String {
    match wait_for_state(runtime, |state| matches!(state, RuntimeState::Down { .. })).await {
        RuntimeState::Down { reason } => reason,
        _ => unreachable!(),
    }
}

fn recorded(folder: &Path, extension: &str) -> String {
    fs::read_to_string(folder.join("model").with_extension(extension)).unwrap_or_default()
}

fn launch_count(folder: &Path) -> usize {
    recorded(folder, "launches").lines().count()
}

fn port_accepts_connections(port: u16) -> bool {
    TcpStream::connect(("127.0.0.1", port)).is_ok()
}

#[tokio::test]
async fn starts_in_the_starting_state() {
    let folder = TempDir::new().unwrap();
    let runtime = Runtime::start(config_for(folder.path(), "loading"));

    assert_eq!(*runtime.watch_state().borrow(), RuntimeState::Starting);
    runtime.stop().await;
}

#[tokio::test]
async fn becomes_ready_with_a_loopback_v1_base_url_once_health_answers_200() {
    let folder = TempDir::new().unwrap();
    let runtime = Runtime::start(config_for(folder.path(), "ready"));

    let port = wait_until_ready(&runtime).await;

    let expected = format!("http://127.0.0.1:{port}/v1");
    assert_eq!(
        *runtime.watch_state().borrow(),
        RuntimeState::Ready {
            base_url: expected.parse().unwrap()
        }
    );
    runtime.stop().await;
}

#[tokio::test]
async fn passes_model_files_context_and_loopback_host_on_the_command_line() {
    let folder = TempDir::new().unwrap();
    let config = config_for(folder.path(), "ready");
    let runtime = Runtime::start(config.clone());

    let port = wait_until_ready(&runtime).await;

    let expected_args = [
        "-m".to_string(),
        config.model_file.display().to_string(),
        "--mmproj".to_string(),
        config.mmproj_file.display().to_string(),
        "-c".to_string(),
        "8192".to_string(),
        "--host".to_string(),
        "127.0.0.1".to_string(),
        "--port".to_string(),
        port.to_string(),
        "--no-ui".to_string(),
        "--jinja".to_string(),
    ]
    .join("\n");
    assert_eq!(recorded(folder.path(), "args"), expected_args);
    runtime.stop().await;
}

#[tokio::test]
async fn hands_the_api_key_to_the_server_through_the_environment_only() {
    let folder = TempDir::new().unwrap();
    let runtime = Runtime::start(config_for(folder.path(), "ready"));

    wait_until_ready(&runtime).await;

    assert_eq!(runtime.api_key().len(), 64);
    assert_eq!(recorded(folder.path(), "api_key"), runtime.api_key());
    assert!(!recorded(folder.path(), "args").contains(runtime.api_key()));
    runtime.stop().await;
}

#[tokio::test]
async fn every_start_gets_a_different_api_key() {
    let first_folder = TempDir::new().unwrap();
    let second_folder = TempDir::new().unwrap();
    let first = Runtime::start(config_for(first_folder.path(), "loading"));
    let second = Runtime::start(config_for(second_folder.path(), "loading"));

    assert_ne!(first.api_key(), second.api_key());
    first.stop().await;
    second.stop().await;
}

#[tokio::test]
async fn restarts_a_crashed_server_and_becomes_ready() {
    let folder = TempDir::new().unwrap();
    let runtime = Runtime::start(config_for(folder.path(), "crash-first-launch"));

    wait_until_ready(&runtime).await;

    assert_eq!(launch_count(folder.path()), 2);
    runtime.stop().await;
}

#[tokio::test]
async fn gives_up_after_the_fourth_crash_within_a_minute() {
    let folder = TempDir::new().unwrap();
    let runtime = Runtime::start(config_for(folder.path(), "crash"));

    let reason = wait_until_down(&runtime).await;

    assert_eq!(launch_count(folder.path()), 4);
    assert!(reason.contains("4 times"), "reason was: {reason}");
    runtime.stop().await;
}

#[tokio::test]
async fn missing_server_binary_is_down_with_the_path_in_the_reason() {
    let folder = TempDir::new().unwrap();
    let mut config = config_for(folder.path(), "ready");
    config.server_binary = folder.path().join("no-such-llama-server");
    let runtime = Runtime::start(config);

    let reason = wait_until_down(&runtime).await;

    assert!(
        reason.contains("no-such-llama-server"),
        "reason was: {reason}"
    );
    runtime.stop().await;
}

#[tokio::test]
async fn stop_kills_a_ready_server() {
    let folder = TempDir::new().unwrap();
    let runtime = Runtime::start(config_for(folder.path(), "ready"));
    let port = wait_until_ready(&runtime).await;
    assert!(port_accepts_connections(port));

    runtime.stop().await;

    assert!(!port_accepts_connections(port));
}

#[tokio::test]
async fn stop_kills_a_server_that_is_still_loading() {
    let folder = TempDir::new().unwrap();
    let runtime = Runtime::start(config_for(folder.path(), "loading"));
    let port = tokio::time::timeout(WAIT_LIMIT, async {
        loop {
            if let Some(port) = recorded(folder.path(), "args")
                .lines()
                .skip_while(|arg| *arg != "--port")
                .nth(1)
            {
                let port: u16 = port.parse().unwrap();
                if port_accepts_connections(port) {
                    return port;
                }
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    })
    .await
    .expect("the fake server never started listening");

    runtime.stop().await;

    assert!(!port_accepts_connections(port));
}
```

- [ ] **Step 3: Run the tests to verify they fail**

Run: `cargo test -p dot-runtime --test runtime`
Expected: compile error `unresolved import` for `dot_runtime::{Runtime, RuntimeConfig, RuntimeState}`.

- [ ] **Step 4: Implement the process and its supervisor**

`crates/dot-runtime/src/lib.rs`:

```rust
//! Keeps one llama-server process running for a local model and says whether it can take requests.
//! Must not know about chat messages, providers, the UI or where Hey Dot keeps its files.

mod runtime;
mod server_process;

pub use runtime::{Runtime, RuntimeConfig, RuntimeState};
```

`crates/dot-runtime/src/server_process.rs`:

```rust
//! Launches one llama-server process and checks whether it is serving.
//! Must not restart, retry or decide anything; the supervisor in `runtime` does that.

use std::fs::OpenOptions;
use std::net::TcpListener;
use std::process::Stdio;

use tokio::process::{Child, Command};

use crate::runtime::RuntimeConfig;

/// Asks the OS for a free port. The port can be taken again before llama-server binds it;
/// that shows up as a crash and the next restart picks a new port.
pub(crate) fn pick_free_port() -> std::io::Result<u16> {
    let listener = TcpListener::bind("127.0.0.1:0")?;
    Ok(listener.local_addr()?.port())
}

pub(crate) fn spawn_server(
    config: &RuntimeConfig,
    port: u16,
    api_key: &str,
) -> std::io::Result<Child> {
    let log_file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&config.log_file)?;
    Command::new(&config.server_binary)
        .arg("-m")
        .arg(&config.model_file)
        .arg("--mmproj")
        .arg(&config.mmproj_file)
        .arg("-c")
        .arg(config.context_tokens.to_string())
        .args([
            "--host",
            "127.0.0.1",
            "--port",
            &port.to_string(),
            "--no-ui",
            "--jinja",
        ])
        // Passed through the environment, not argv, so `ps` does not show it to other users.
        .env("LLAMA_API_KEY", api_key)
        .stdin(Stdio::null())
        .stdout(log_file.try_clone()?)
        .stderr(log_file)
        .kill_on_drop(true)
        .spawn()
}

/// llama-server answers 200 on `/health` only once the model is loaded. While loading it
/// answers other codes or refuses the connection, which all mean "not ready yet".
pub(crate) async fn is_serving(client: &reqwest::Client, port: u16) -> bool {
    let response = client
        .get(format!("http://127.0.0.1:{port}/health"))
        .send()
        .await;
    matches!(response, Ok(response) if response.status() == reqwest::StatusCode::OK)
}
```

`crates/dot-runtime/src/runtime.rs`:

```rust
//! Supervises llama-server: start, wait for health, restart on crash, stop.
//! Must not read settings or pick the model; the caller passes every path.

use std::collections::VecDeque;
use std::path::PathBuf;
use std::time::Duration;

use tokio::sync::{oneshot, watch};
use tokio::task::JoinHandle;
use tokio::time::Instant;
use url::Url;

use crate::server_process::{is_serving, pick_free_port, spawn_server};

const HEALTH_POLL_INTERVAL: Duration = Duration::from_millis(250);
const HEALTH_REQUEST_TIMEOUT: Duration = Duration::from_secs(1);
const CRASH_WINDOW: Duration = Duration::from_secs(60);
const MAX_RESTARTS_IN_WINDOW: usize = 3;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeConfig {
    pub server_binary: PathBuf,
    pub model_file: PathBuf,
    pub mmproj_file: PathBuf,
    pub context_tokens: u32,
    /// llama-server's stdout and stderr are appended here. Its folder must exist.
    pub log_file: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeState {
    Starting,
    /// `base_url` is the OpenAI-compatible root, `http://127.0.0.1:<port>/v1`.
    Ready {
        base_url: Url,
    },
    Down {
        reason: String,
    },
}

pub struct Runtime {
    api_key: String,
    state: watch::Receiver<RuntimeState>,
    stop_signal: oneshot::Sender<()>,
    supervisor: JoinHandle<()>,
}

impl Runtime {
    /// Starts supervising in the background. Must be called inside a tokio runtime.
    pub fn start(config: RuntimeConfig) -> Runtime {
        let api_key = generate_api_key();
        let (state_sender, state) = watch::channel(RuntimeState::Starting);
        let (stop_signal, stop_requested) = oneshot::channel();
        let supervisor = tokio::spawn(supervise(
            config,
            api_key.clone(),
            state_sender,
            stop_requested,
        ));
        Runtime {
            api_key,
            state,
            stop_signal,
            supervisor,
        }
    }

    /// The bearer token every request to this server must carry. New for each `start`.
    pub fn api_key(&self) -> &str {
        &self.api_key
    }

    pub fn watch_state(&self) -> watch::Receiver<RuntimeState> {
        self.state.clone()
    }

    /// Kills llama-server and waits until it has exited.
    pub async fn stop(self) {
        let _ = self.stop_signal.send(());
        let _ = self.supervisor.await;
    }
}

fn generate_api_key() -> String {
    let mut key_bytes = [0u8; 32];
    getrandom::fill(&mut key_bytes).expect("the OS random number generator is unavailable");
    hex::encode(key_bytes)
}

async fn supervise(
    config: RuntimeConfig,
    api_key: String,
    state: watch::Sender<RuntimeState>,
    mut stop_requested: oneshot::Receiver<()>,
) {
    let health_client = reqwest::Client::builder()
        .timeout(HEALTH_REQUEST_TIMEOUT)
        .build()
        .expect("a client with only a timeout always builds");
    let mut recent_crashes: VecDeque<Instant> = VecDeque::new();
    loop {
        state.send_replace(RuntimeState::Starting);
        let launch =
            pick_free_port().and_then(|port| Ok((port, spawn_server(&config, port, &api_key)?)));
        let (port, mut server) = match launch {
            Ok(launched) => launched,
            Err(error) => {
                state.send_replace(RuntimeState::Down {
                    reason: format!(
                        "could not start {}: {error}",
                        config.server_binary.display()
                    ),
                });
                return;
            }
        };

        let mut announced_ready = false;
        let mut health_poll = tokio::time::interval(HEALTH_POLL_INTERVAL);
        let exit = loop {
            tokio::select! {
                // Also fires when the Runtime is dropped without `stop`, so the server never outlives it.
                _ = &mut stop_requested => {
                    let _ = server.kill().await;
                    return;
                }
                exit = server.wait() => break exit,
                _ = health_poll.tick(), if !announced_ready => {
                    if is_serving(&health_client, port).await {
                        let base_url = Url::parse(&format!("http://127.0.0.1:{port}/v1"))
                            .expect("a loopback URL with a port always parses");
                        state.send_replace(RuntimeState::Ready { base_url });
                        announced_ready = true;
                    }
                }
            }
        };

        let now = Instant::now();
        recent_crashes.push_back(now);
        recent_crashes.retain(|crashed_at| now.duration_since(*crashed_at) < CRASH_WINDOW);
        if recent_crashes.len() > MAX_RESTARTS_IN_WINDOW {
            let exit = exit.map_or_else(|error| error.to_string(), |status| status.to_string());
            state.send_replace(RuntimeState::Down {
                reason: format!(
                    "llama-server stopped {} times within a minute (last: {exit})",
                    recent_crashes.len()
                ),
            });
            return;
        }
    }
}
```

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test -p dot-runtime --test runtime`
Expected: `test result: ok. 10 passed; 0 failed`. The crash tests take about a second each; nothing waits for a timeout.

- [ ] **Step 6: Write the module doc and route to it**

`crates/dot-runtime/AGENTS.md` (Task 7 adds the real-model bullet):

```markdown
# dot-runtime

Owns: one supervised llama-server process for a local model: picking its port, generating its API key, starting it, polling `/health` until it serves, restarting it after a crash, giving up after repeated crashes, and killing it on stop.

Must not know about: chat messages, providers, the UI, settings, the model catalog or where Hey Dot keeps its files. The caller passes the binary, model, projector and log paths.

Entry points: `Runtime::start(RuntimeConfig)`, then `watch_state()` for `Starting` / `Ready { base_url }` / `Down { reason }`, `api_key()` for the bearer token, and `stop().await`.

Invariants and gotchas:
- `Runtime::start` must run inside a tokio runtime; in the app that means `tauri::async_runtime`.
- The server listens on `127.0.0.1` only, on a fresh free port for every launch. llama-server binds its port only after the model loaded, so a port race shows up as a crash and the restart picks another port.
- The API key is 32 random bytes as hex, new for every `start`, and reaches llama-server through the `LLAMA_API_KEY` environment variable so it never shows in `ps`.
- `Ready` means `/health` answered 200. While the model loads llama-server refuses connections or answers 503.
- A crash restarts at once. The fourth exit within 60 seconds is `Down` with the last exit status, and nothing restarts after that until a new `start`.
- `stop` and dropping the `Runtime` both kill the server, including one still loading. A SIGKILL of the whole app leaves llama-server running as an orphan until it is killed by hand; it cannot answer anyone without the key.
- stdout and stderr are appended to `log_file`, whose folder must exist. Nothing rotates it yet.
- Tests run `fake-llama-server` (`tests/support/`), a std-only stand-in whose model file text picks ready, loading, crash or crash-first-launch. It is never shipped.

Called by: `app/src-tauri` (`local_model.rs`).
```

In the root `AGENTS.md`, after the `crates/dot-models/` line, add:

```markdown
- `crates/dot-runtime/`: supervised llama-server process. See `crates/dot-runtime/AGENTS.md`.
```

- [ ] **Step 7: Format, lint, commit**

```bash
cargo fmt --all
cargo clippy -p dot-runtime --all-targets -- -D warnings
cargo test -p dot-runtime
git add crates/dot-runtime AGENTS.md Cargo.lock
git commit -m "Add dot-runtime: start, health-check, restart and stop llama-server"
```

Expected: no clippy warnings; `10 passed`.

---

### Task 4: Build and bundle the pinned llama-server

**Files:**
- Create: `scripts/build-llama-server.sh` (executable)
- Modify: `.gitignore`
- Modify: `app/src-tauri/tauri.conf.json` (`bundle.externalBin`, `bundle.resources`)
- Modify: `.github/workflows/ci.yml` (both jobs)
- Modify: `app/AGENTS.md`, `AGENTS.md`

**Interfaces:**
- Consumes: nothing.
- Produces: `app/src-tauri/binaries/llama-server-{aarch64,x86_64,universal}-apple-darwin` and `app/src-tauri/licenses/llama.cpp/LICENSE-*`, both git-ignored. A bundled app has `Contents/MacOS/llama-server` next to `Contents/MacOS/hey-dot`, and a dev build has `target/debug/llama-server`: Task 5 finds the binary at `current_exe().with_file_name("llama-server")`. Task 7's real-model test runs `app/src-tauri/binaries/llama-server-aarch64-apple-darwin`.

- [ ] **Step 1: Add the bundle config and watch the build fail**

In `app/src-tauri/tauri.conf.json`, after the `"icon": [...]` array inside `"bundle"`, add:

```json
    "externalBin": [
      "binaries/llama-server"
    ],
    "resources": {
      "licenses/llama.cpp/": "licenses/llama.cpp/"
    }
```

Tauri appends the target triple to `externalBin` paths at build time and strips it when copying the file into the bundle.

Run: `cargo check -p hey-dot`
Expected: FAIL: ``failed to run custom build command for `hey-dot` `` with ``resource path `binaries/llama-server-aarch64-apple-darwin` doesn't exist``. With the binary present but the licenses missing it names `licenses/llama.cpp` instead. This is the gotcha every later cargo command depends on.

- [ ] **Step 2: Write the build script**

`scripts/build-llama-server.sh`:

```bash
#!/usr/bin/env bash
# Builds the pinned llama-server that the app bundles as its Tauri externalBin, plus the
# license files it must ship with. Must not install anything outside this repository.
set -euo pipefail

LLAMA_CPP_TAG="b11123"

repo_root="$(cd "$(dirname "$0")/.." && pwd)"
source_dir="$repo_root/target/llama.cpp-$LLAMA_CPP_TAG"
binaries_dir="$repo_root/app/src-tauri/binaries"
licenses_dir="$repo_root/app/src-tauri/licenses/llama.cpp"

if [ ! -d "$source_dir" ]; then
  git clone --depth 1 --branch "$LLAMA_CPP_TAG" https://github.com/ggml-org/llama.cpp.git "$source_dir"
fi

common_flags=(
  -DCMAKE_BUILD_TYPE=Release
  -DBUILD_SHARED_LIBS=OFF
  -DGGML_NATIVE=OFF
  -DLLAMA_BUILD_TESTS=OFF
  -DLLAMA_BUILD_EXAMPLES=OFF
  -DLLAMA_BUILD_TOOLS=ON
  -DLLAMA_BUILD_SERVER=ON
  -DLLAMA_OPENSSL=OFF
  -DCMAKE_OSX_DEPLOYMENT_TARGET=13.3
)

# The Metal shaders are embedded in the binary, so no Metal compiler (full Xcode) is needed.
cmake -S "$source_dir" -B "$source_dir/build-arm64" "${common_flags[@]}" \
  -DCMAKE_OSX_ARCHITECTURES=arm64 -DGGML_METAL=ON -DGGML_METAL_EMBED_LIBRARY=ON
cmake --build "$source_dir/build-arm64" --config Release --target llama-server -j "$(sysctl -n hw.ncpu)"

cmake -S "$source_dir" -B "$source_dir/build-x86_64" "${common_flags[@]}" \
  -DCMAKE_OSX_ARCHITECTURES=x86_64 -DGGML_METAL=OFF
cmake --build "$source_dir/build-x86_64" --config Release --target llama-server -j "$(sysctl -n hw.ncpu)"

# Tauri needs one file per target triple: cargo builds each architecture of a universal
# app separately, and the bundler then looks for the universal file.
mkdir -p "$binaries_dir"
cp "$source_dir/build-arm64/bin/llama-server" "$binaries_dir/llama-server-aarch64-apple-darwin"
cp "$source_dir/build-x86_64/bin/llama-server" "$binaries_dir/llama-server-x86_64-apple-darwin"
lipo -create -output "$binaries_dir/llama-server-universal-apple-darwin" \
  "$binaries_dir/llama-server-aarch64-apple-darwin" "$binaries_dir/llama-server-x86_64-apple-darwin"

mkdir -p "$licenses_dir"
cp "$source_dir/LICENSE" "$licenses_dir/LICENSE-llama.cpp"
cp "$source_dir/vendor/cpp-httplib/LICENSE" "$licenses_dir/LICENSE-cpp-httplib"
cp "$source_dir/licenses/"* "$licenses_dir/"
```

```bash
chmod +x scripts/build-llama-server.sh
```

Add to `.gitignore`:

```
/app/src-tauri/binaries/
/app/src-tauri/licenses/
```

- [ ] **Step 3: Build llama-server and check the output**

Run: `time scripts/build-llama-server.sh && lipo -archs app/src-tauri/binaries/llama-server-universal-apple-darwin && app/src-tauri/binaries/llama-server-aarch64-apple-darwin --version && ls app/src-tauri/licenses/llama.cpp`
Expected: about 1.5 minutes on an M-series Mac (measured 1:35, clone included); `x86_64 arm64`; `version: ... (build ..., commit 384a534)`; `LICENSE-cpp-httplib  LICENSE-jsonhpp  LICENSE-llama.cpp`. The arm64 file is about 17.9 MB, the x86_64 file about 17.0 MB, the universal file about 34.9 MB. `otool -L` on the arm64 file lists only system libraries and frameworks (Metal, Accelerate, Foundation).

- [ ] **Step 4: Verify the build and the bundle layout**

Run: `cargo check -p hey-dot && pnpm -C app tauri build --debug --bundles app && ls "target/debug/bundle/macos/Hey Dot.app/Contents/MacOS" "target/debug/bundle/macos/Hey Dot.app/Contents/Resources/licenses/llama.cpp" && ls target/debug/llama-server`
Expected: `Finished`; `MacOS` lists `hey-dot` and `llama-server`; the licenses folder lists the three files; `target/debug/llama-server` exists.

- [ ] **Step 5: Build or restore llama-server in CI**

In `.github/workflows/ci.yml`, in both jobs, right after `- uses: actions/checkout@v7`, add:

```yaml
      - id: llama-server-cache
        uses: actions/cache@v6.1.0
        with:
          path: |
            app/src-tauri/binaries
            app/src-tauri/licenses
          key: llama-server-${{ hashFiles('scripts/build-llama-server.sh') }}
      # tauri-build fails every cargo command on hey-dot while the externalBin or resources are missing
      - if: steps.llama-server-cache.outputs.cache-hit != 'true'
        run: scripts/build-llama-server.sh
```

GitHub's macOS runners ship CMake and git. Changing the pinned tag changes the script, which changes the cache key.

- [ ] **Step 6: Update the docs**

In `app/AGENTS.md`, after the bundle identifier bullet, add:

```markdown
- `scripts/build-llama-server.sh` must have run before any cargo command on `hey-dot`: tauri-build fails the build while `src-tauri/binaries/llama-server-<target triple>` or `src-tauri/licenses/llama.cpp/` is missing. Both folders are git-ignored build output.
```

In the root `AGENTS.md`, after the `crates/dot-providers/` line, add:

```markdown
- `scripts/build-llama-server.sh`: builds the pinned llama-server the app bundles, plus its licenses.
```

make the Run section start with:

```
    scripts/build-llama-server.sh   # once, and after changing it; needs cmake and git
```

and under the `pnpm -C app build` line of the Test section add:

```
                             # and scripts/build-llama-server.sh must have run once
```

- [ ] **Step 7: Commit**

```bash
git add scripts/build-llama-server.sh .gitignore app/src-tauri/tauri.conf.json app/AGENTS.md AGENTS.md
git commit -m "Build the pinned llama-server and bundle it with its licenses"
git add .github/workflows/ci.yml
git commit -m "Build or restore llama-server in CI before any cargo step"
```

---

### Task 5: The app keeps the local model running and answers over IPC

**Files:**
- Modify: `app/src-tauri/Cargo.toml`
- Create: `app/src-tauri/src/local_model.rs`
- Create: `app/src-tauri/src/commands.rs`
- Modify: `app/src-tauri/src/lib.rs`
- Modify: `app/AGENTS.md`
- Modify: `Cargo.lock`

**Interfaces:**
- Consumes: Task 1 `installed_chat_model`, Task 2 `ChatConfig`, `ChatMessage`, `ChatRole`, `stream_chat`, Task 3 `Runtime`, `RuntimeConfig`, `RuntimeState`; step 2's `download_model`, `DownloadProgress`, `detect_hardware`, `recommend`, `Recommendation`, `ModelChoice`.
- Produces, the IPC contract Task 6 mirrors in TypeScript:
  - command `watch_local_model({ onStatus: Channel<LocalModelStatus> })`, sends the current status at once and on every change.
  - command `download_local_model()`, returns at once; progress arrives as statuses.
  - command `ask_text({ question: string, onEvent: Channel<AnswerEvent> })`, resolves after the last delta, rejects with the error text.
  - `LocalModelStatus` JSON: `{ "modelName": string, "downloadBytes": number, "phase": { "kind": "notInstalled" | "downloading" (+ "percent") | "downloadFailed" (+ "reason") | "starting" | "ready" | "down" (+ "reason") } }`.
  - `AnswerEvent` JSON: `{ "event": "started", "data": { "leavesDevice": bool, "host": string } }` or `{ "event": "delta", "data": { "text": string } }`.

- [ ] **Step 1: Add the dependencies**

Append to `[dependencies]` in `app/src-tauri/Cargo.toml`, and add the `[dev-dependencies]` section:

```toml
dot-models = { path = "../../crates/dot-models" }
dot-providers = { path = "../../crates/dot-providers" }
dot-runtime = { path = "../../crates/dot-runtime" }
futures-util = "0.3.34"
reqwest = "0.13.5"
tokio = { version = "1.53.1", features = ["sync"] }

[dev-dependencies]
tempfile = "3.27.0"
tokio = { version = "1.53.1", features = ["macros", "rt-multi-thread", "time"] }
```

Run: `cargo check -p hey-dot`
Expected: `Finished`.

- [ ] **Step 2: Write the failing `LocalModel` tests**

Create `app/src-tauri/src/local_model.rs` with the header and the tests only:

```rust
//! Keeps the chosen local chat model usable: its files on disk and llama-server serving it.
//! Must not know about questions, answers or cloud providers.

#[cfg(test)]
mod tests {
    use std::net::TcpListener;
    use std::time::Duration;

    use dot_models::{Chip, Hardware, ModelFile, QWEN3_VL_2B, QWEN3_VL_4B, recommend};

    use super::*;

    const GIB: u64 = 1024 * 1024 * 1024;

    fn local_model_downloading_from(url: String, models_dir: PathBuf) -> Arc<LocalModel> {
        let files = vec![ModelFile {
            name: "weights.gguf",
            url: url.leak(),
            sha256: "0000000000000000000000000000000000000000000000000000000000000000",
            bytes: 1000,
        }];
        let model = Box::leak(Box::new(Model {
            id: "test-model",
            display_name: "Test model",
            min_ram_gb: 0,
            files: files.leak(),
        }));
        let paths = LocalModelPaths {
            models_dir,
            server_binary: PathBuf::from("llama-server"),
            log_file: PathBuf::from("llama-server.log"),
        };
        Arc::new(LocalModel::new(model, paths, reqwest::Client::new()))
    }

    #[tokio::test]
    async fn a_model_that_is_not_running_has_no_chat_config() {
        let models_dir = tempfile::tempdir().unwrap();
        let local_model = local_model_downloading_from(
            "http://127.0.0.1:9/".to_owned(),
            models_dir.path().into(),
        );
        assert_eq!(
            local_model.subscribe().borrow().phase,
            LocalModelPhase::NotInstalled
        );
        assert_eq!(local_model.chat_config().await, None);
    }

    #[tokio::test]
    async fn a_second_download_while_one_runs_returns_at_once() {
        // Never accepted: the connection sits in the backlog, so the first download hangs.
        let silent_server = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!(
            "http://{}/weights.gguf",
            silent_server.local_addr().unwrap()
        );
        let models_dir = tempfile::tempdir().unwrap();
        let local_model = local_model_downloading_from(url, models_dir.path().into());
        let first_download = tokio::spawn({
            let local_model = Arc::clone(&local_model);
            async move { local_model.download().await }
        });
        let mut status = local_model.subscribe();
        status
            .wait_for(|status| matches!(status.phase, LocalModelPhase::Downloading { .. }))
            .await
            .unwrap();

        tokio::time::timeout(Duration::from_secs(1), local_model.download())
            .await
            .expect("the second download waited for the first");
        assert_eq!(
            status.borrow().phase,
            LocalModelPhase::Downloading { percent: 0 }
        );
        first_download.abort();
    }

    fn downloading_status(percent: u8) -> LocalModelStatus {
        LocalModelStatus {
            model_name: "Test model",
            download_bytes: 1000,
            phase: LocalModelPhase::Downloading { percent },
        }
    }

    #[test]
    fn download_percent_changes_only_on_a_new_whole_percent() {
        let mut status = downloading_status(0);
        let at = |downloaded_bytes| DownloadProgress {
            downloaded_bytes,
            total_bytes: 1000,
        };
        assert!(!set_download_percent(&mut status, &at(9)));
        assert!(set_download_percent(&mut status, &at(10)));
        assert_eq!(status, downloading_status(1));
        assert!(!set_download_percent(&mut status, &at(19)));
        assert!(set_download_percent(&mut status, &at(1000)));
        assert_eq!(status, downloading_status(100));
    }

    #[test]
    fn intel_macs_get_the_2b_model_behind_the_cloud_recommendation() {
        let intel = Hardware {
            chip: Chip::Intel,
            ram_bytes: 16 * GIB,
        };
        assert_eq!(pick_local_model(&recommend(&intel)), &QWEN3_VL_2B);
    }

    #[test]
    fn a_16_gb_apple_silicon_mac_gets_the_4b_model() {
        let apple_silicon = Hardware {
            chip: Chip::AppleSilicon,
            ram_bytes: 16 * GIB,
        };
        assert_eq!(pick_local_model(&recommend(&apple_silicon)), &QWEN3_VL_4B);
    }

    #[test]
    fn status_serializes_in_the_shape_the_chat_panel_reads() {
        let status = LocalModelStatus {
            model_name: "Qwen3-VL 4B",
            download_bytes: 3,
            phase: LocalModelPhase::DownloadFailed {
                reason: "disk full".to_owned(),
            },
        };
        assert_eq!(
            serde_json::to_value(&status).unwrap(),
            serde_json::json!({
                "modelName": "Qwen3-VL 4B",
                "downloadBytes": 3,
                "phase": {"kind": "downloadFailed", "reason": "disk full"}
            })
        );
    }
}
```

Add `mod local_model;` as the first line of `app/src-tauri/src/lib.rs`.

- [ ] **Step 3: Run the tests to verify they fail**

Run: `cargo test -p hey-dot --lib local_model`
Expected: compile errors: cannot find `LocalModel`, `LocalModelPaths`, `LocalModelStatus`, `LocalModelPhase`, `set_download_percent`, `pick_local_model`, and unresolved `super::*` items.

- [ ] **Step 4: Implement `LocalModel`**

Put this above the test module, under the header, so the whole file reads:

```rust
//! Keeps the chosen local chat model usable: its files on disk and llama-server serving it.
//! Must not know about questions, answers or cloud providers.

use std::path::PathBuf;
use std::sync::Arc;

use dot_models::{
    DownloadProgress, Model, ModelChoice, Recommendation, download_model, installed_chat_model,
};
use dot_providers::ChatConfig;
use dot_runtime::{Runtime, RuntimeConfig, RuntimeState};
use serde::Serialize;
use tokio::sync::{Mutex, watch};

const CONTEXT_TOKENS: u32 = 8192;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalModelStatus {
    pub model_name: &'static str,
    pub download_bytes: u64,
    pub phase: LocalModelPhase,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum LocalModelPhase {
    NotInstalled,
    Downloading { percent: u8 },
    DownloadFailed { reason: String },
    Starting,
    Ready,
    Down { reason: String },
}

pub struct LocalModelPaths {
    pub models_dir: PathBuf,
    pub server_binary: PathBuf,
    pub log_file: PathBuf,
}

pub struct LocalModel {
    model: &'static Model,
    paths: LocalModelPaths,
    http: reqwest::Client,
    status: watch::Sender<LocalModelStatus>,
    runtime: Mutex<Option<Runtime>>,
    download_lock: Mutex<()>,
}

impl LocalModel {
    pub fn new(model: &'static Model, paths: LocalModelPaths, http: reqwest::Client) -> LocalModel {
        let phase = match installed_chat_model(model, &paths.models_dir) {
            Some(_) => LocalModelPhase::Starting,
            None => LocalModelPhase::NotInstalled,
        };
        let status = LocalModelStatus {
            model_name: model.display_name,
            download_bytes: model.files.iter().map(|file| file.bytes).sum(),
            phase,
        };
        LocalModel {
            model,
            paths,
            http,
            status: watch::Sender::new(status),
            runtime: Mutex::new(None),
            download_lock: Mutex::new(()),
        }
    }

    pub fn subscribe(&self) -> watch::Receiver<LocalModelStatus> {
        self.status.subscribe()
    }

    /// Starts llama-server unless the files are missing or it is already running.
    pub async fn start_if_installed(self: &Arc<Self>) {
        let Some(files) = installed_chat_model(self.model, &self.paths.models_dir) else {
            return;
        };
        let mut runtime_slot = self.runtime.lock().await;
        if runtime_slot.is_some() {
            return;
        }
        let runtime = Runtime::start(RuntimeConfig {
            server_binary: self.paths.server_binary.clone(),
            model_file: files.weights,
            mmproj_file: files.projector,
            context_tokens: CONTEXT_TOKENS,
            log_file: self.paths.log_file.clone(),
        });
        let mut runtime_state = runtime.watch_state();
        *runtime_slot = Some(runtime);
        let local_model = Arc::clone(self);
        tokio::spawn(async move {
            loop {
                let phase = match &*runtime_state.borrow_and_update() {
                    RuntimeState::Starting => LocalModelPhase::Starting,
                    RuntimeState::Ready { .. } => LocalModelPhase::Ready,
                    RuntimeState::Down { reason } => LocalModelPhase::Down {
                        reason: reason.clone(),
                    },
                };
                local_model
                    .status
                    .send_modify(|status| status.phase = phase);
                if runtime_state.changed().await.is_err() {
                    return;
                }
            }
        });
    }

    /// Downloads the model and starts it. A second call while one runs does nothing.
    pub async fn download(self: &Arc<Self>) {
        let Ok(_download_guard) = self.download_lock.try_lock() else {
            return;
        };
        self.status
            .send_modify(|status| status.phase = LocalModelPhase::Downloading { percent: 0 });
        let downloaded =
            download_model(&self.http, self.model, &self.paths.models_dir, |progress| {
                self.status
                    .send_if_modified(|status| set_download_percent(status, &progress));
            })
            .await;
        match downloaded {
            Ok(_) => self.start_if_installed().await,
            Err(error) => self.status.send_modify(|status| {
                status.phase = LocalModelPhase::DownloadFailed {
                    reason: error.to_string(),
                }
            }),
        }
    }

    /// Where to send a chat request, or `None` until llama-server is ready.
    pub async fn chat_config(&self) -> Option<ChatConfig> {
        let runtime_slot = self.runtime.lock().await;
        let runtime = runtime_slot.as_ref()?;
        let RuntimeState::Ready { base_url } = runtime.watch_state().borrow().clone() else {
            return None;
        };
        Some(ChatConfig {
            base_url,
            api_key: runtime.api_key().to_owned(),
            model: self.model.id.to_owned(),
        })
    }

    pub async fn shutdown(&self) {
        if let Some(runtime) = self.runtime.lock().await.take() {
            runtime.stop().await;
        }
    }
}

/// Returns whether the whole percent changed, so the UI hears at most 100 updates.
fn set_download_percent(status: &mut LocalModelStatus, progress: &DownloadProgress) -> bool {
    let percent = (progress.downloaded_bytes * 100 / progress.total_bytes.max(1)) as u8;
    let phase = LocalModelPhase::Downloading { percent };
    if status.phase == phase {
        return false;
    }
    status.phase = phase;
    true
}

/// The local model this Mac gets until onboarding lets the user choose (Phase 1 step 8).
pub fn pick_local_model(recommendation: &Recommendation) -> &'static Model {
    std::iter::once(&recommendation.recommended)
        .chain(&recommendation.also_offered)
        .find_map(|choice| match choice {
            ModelChoice::Local(model) => Some(*model),
            ModelChoice::Cloud => None,
        })
        .expect("every recommendation offers a local model")
}

#[cfg(test)]
mod tests {
    use std::net::TcpListener;
    use std::time::Duration;

    use dot_models::{Chip, Hardware, ModelFile, QWEN3_VL_2B, QWEN3_VL_4B, recommend};

    use super::*;

    const GIB: u64 = 1024 * 1024 * 1024;

    fn local_model_downloading_from(url: String, models_dir: PathBuf) -> Arc<LocalModel> {
        let files = vec![ModelFile {
            name: "weights.gguf",
            url: url.leak(),
            sha256: "0000000000000000000000000000000000000000000000000000000000000000",
            bytes: 1000,
        }];
        let model = Box::leak(Box::new(Model {
            id: "test-model",
            display_name: "Test model",
            min_ram_gb: 0,
            files: files.leak(),
        }));
        let paths = LocalModelPaths {
            models_dir,
            server_binary: PathBuf::from("llama-server"),
            log_file: PathBuf::from("llama-server.log"),
        };
        Arc::new(LocalModel::new(model, paths, reqwest::Client::new()))
    }

    #[tokio::test]
    async fn a_model_that_is_not_running_has_no_chat_config() {
        let models_dir = tempfile::tempdir().unwrap();
        let local_model = local_model_downloading_from(
            "http://127.0.0.1:9/".to_owned(),
            models_dir.path().into(),
        );
        assert_eq!(
            local_model.subscribe().borrow().phase,
            LocalModelPhase::NotInstalled
        );
        assert_eq!(local_model.chat_config().await, None);
    }

    #[tokio::test]
    async fn a_second_download_while_one_runs_returns_at_once() {
        // Never accepted: the connection sits in the backlog, so the first download hangs.
        let silent_server = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!(
            "http://{}/weights.gguf",
            silent_server.local_addr().unwrap()
        );
        let models_dir = tempfile::tempdir().unwrap();
        let local_model = local_model_downloading_from(url, models_dir.path().into());
        let first_download = tokio::spawn({
            let local_model = Arc::clone(&local_model);
            async move { local_model.download().await }
        });
        let mut status = local_model.subscribe();
        status
            .wait_for(|status| matches!(status.phase, LocalModelPhase::Downloading { .. }))
            .await
            .unwrap();

        tokio::time::timeout(Duration::from_secs(1), local_model.download())
            .await
            .expect("the second download waited for the first");
        assert_eq!(
            status.borrow().phase,
            LocalModelPhase::Downloading { percent: 0 }
        );
        first_download.abort();
    }

    fn downloading_status(percent: u8) -> LocalModelStatus {
        LocalModelStatus {
            model_name: "Test model",
            download_bytes: 1000,
            phase: LocalModelPhase::Downloading { percent },
        }
    }

    #[test]
    fn download_percent_changes_only_on_a_new_whole_percent() {
        let mut status = downloading_status(0);
        let at = |downloaded_bytes| DownloadProgress {
            downloaded_bytes,
            total_bytes: 1000,
        };
        assert!(!set_download_percent(&mut status, &at(9)));
        assert!(set_download_percent(&mut status, &at(10)));
        assert_eq!(status, downloading_status(1));
        assert!(!set_download_percent(&mut status, &at(19)));
        assert!(set_download_percent(&mut status, &at(1000)));
        assert_eq!(status, downloading_status(100));
    }

    #[test]
    fn intel_macs_get_the_2b_model_behind_the_cloud_recommendation() {
        let intel = Hardware {
            chip: Chip::Intel,
            ram_bytes: 16 * GIB,
        };
        assert_eq!(pick_local_model(&recommend(&intel)), &QWEN3_VL_2B);
    }

    #[test]
    fn a_16_gb_apple_silicon_mac_gets_the_4b_model() {
        let apple_silicon = Hardware {
            chip: Chip::AppleSilicon,
            ram_bytes: 16 * GIB,
        };
        assert_eq!(pick_local_model(&recommend(&apple_silicon)), &QWEN3_VL_4B);
    }

    #[test]
    fn status_serializes_in_the_shape_the_chat_panel_reads() {
        let status = LocalModelStatus {
            model_name: "Qwen3-VL 4B",
            download_bytes: 3,
            phase: LocalModelPhase::DownloadFailed {
                reason: "disk full".to_owned(),
            },
        };
        assert_eq!(
            serde_json::to_value(&status).unwrap(),
            serde_json::json!({
                "modelName": "Qwen3-VL 4B",
                "downloadBytes": 3,
                "phase": {"kind": "downloadFailed", "reason": "disk full"}
            })
        );
    }
}
```

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test -p hey-dot --lib local_model`
Expected: `6 passed`. `dead_code` warnings are expected until Step 9 wires `LocalModel` into the app.

- [ ] **Step 6: Write the failing command-shape test**

Create `app/src-tauri/src/commands.rs` with the header and the test only:

```rust
//! The IPC commands the chat panel calls.
//! Must not hold state: everything lives in the managed `LocalModel` and HTTP client.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn answer_events_serialize_in_the_shape_the_chat_panel_reads() {
        let started = AnswerEvent::Started {
            leaves_device: false,
            host: "127.0.0.1".to_owned(),
        };
        assert_eq!(
            serde_json::to_value(&started).unwrap(),
            serde_json::json!({"event": "started", "data": {"leavesDevice": false, "host": "127.0.0.1"}})
        );
        assert_eq!(
            serde_json::to_value(AnswerEvent::Delta {
                text: "Par".to_owned()
            })
            .unwrap(),
            serde_json::json!({"event": "delta", "data": {"text": "Par"}})
        );
    }
}
```

Add `mod commands;` above `mod local_model;` in `app/src-tauri/src/lib.rs`.

- [ ] **Step 7: Run the test to verify it fails**

Run: `cargo test -p hey-dot --lib commands`
Expected: compile error: cannot find type `AnswerEvent`.

- [ ] **Step 8: Implement the commands**

`app/src-tauri/src/commands.rs`:

```rust
//! The IPC commands the chat panel calls.
//! Must not hold state: everything lives in the managed `LocalModel` and HTTP client.

use std::sync::Arc;

use dot_providers::{ChatMessage, ChatRole, stream_chat};
use futures_util::StreamExt;
use serde::Serialize;
use tauri::State;
use tauri::ipc::Channel;

use crate::local_model::{LocalModel, LocalModelStatus};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    tag = "event",
    content = "data"
)]
pub enum AnswerEvent {
    Started { leaves_device: bool, host: String },
    Delta { text: String },
}

/// Sends the current status now and every change after it, until the webview goes away.
#[tauri::command]
pub fn watch_local_model(
    local_model: State<'_, Arc<LocalModel>>,
    on_status: Channel<LocalModelStatus>,
) {
    let mut status = local_model.subscribe();
    tauri::async_runtime::spawn(async move {
        loop {
            let current = status.borrow_and_update().clone();
            if on_status.send(current).is_err() || status.changed().await.is_err() {
                return;
            }
        }
    });
}

/// Returns at once; progress arrives through `watch_local_model`.
#[tauri::command]
pub fn download_local_model(local_model: State<'_, Arc<LocalModel>>) {
    let local_model = Arc::clone(&local_model);
    tauri::async_runtime::spawn(async move { local_model.download().await });
}

/// Resolves once the whole answer was sent, so the panel needs no end-of-answer event.
#[tauri::command]
pub async fn ask_text(
    question: String,
    local_model: State<'_, Arc<LocalModel>>,
    http: State<'_, reqwest::Client>,
    on_event: Channel<AnswerEvent>,
) -> Result<(), String> {
    let config = local_model
        .chat_config()
        .await
        .ok_or("The local model is not ready yet.")?;
    let started = AnswerEvent::Started {
        leaves_device: config.leaves_device(),
        host: config.base_url.host_str().unwrap_or_default().to_owned(),
    };
    on_event.send(started).map_err(|error| error.to_string())?;
    let messages = [ChatMessage {
        role: ChatRole::User,
        text: question,
        jpeg_image: None,
    }];
    let mut answer = std::pin::pin!(stream_chat(&http, &config, &messages));
    while let Some(delta) = answer.next().await {
        let text = delta.map_err(|error| error.to_string())?;
        on_event
            .send(AnswerEvent::Delta { text })
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn answer_events_serialize_in_the_shape_the_chat_panel_reads() {
        let started = AnswerEvent::Started {
            leaves_device: false,
            host: "127.0.0.1".to_owned(),
        };
        assert_eq!(
            serde_json::to_value(&started).unwrap(),
            serde_json::json!({"event": "started", "data": {"leavesDevice": false, "host": "127.0.0.1"}})
        );
        assert_eq!(
            serde_json::to_value(AnswerEvent::Delta {
                text: "Par".to_owned()
            })
            .unwrap(),
            serde_json::json!({"event": "delta", "data": {"text": "Par"}})
        );
    }
}
```

- [ ] **Step 9: Wire the app**

`app/src-tauri/src/lib.rs`:

```rust
mod commands;
mod local_model;

use std::sync::Arc;
use std::time::Duration;

use dot_models::{detect_hardware, recommend};
use tauri::{Manager, RunEvent};

use crate::local_model::{LocalModel, LocalModelPaths, pick_local_model};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let http = reqwest::Client::builder()
                .connect_timeout(Duration::from_secs(10))
                .read_timeout(Duration::from_secs(60))
                .build()?;
            let log_dir = app.path().home_dir()?.join("Library/Logs/Hey Dot");
            std::fs::create_dir_all(&log_dir)?;
            let paths = LocalModelPaths {
                models_dir: app.path().data_dir()?.join("Hey Dot/models"),
                // Tauri copies externalBin next to the app's own executable, without the target triple.
                server_binary: std::env::current_exe()?.with_file_name("llama-server"),
                log_file: log_dir.join("llama-server.log"),
            };
            let model = pick_local_model(&recommend(&detect_hardware()));
            let local_model = Arc::new(LocalModel::new(model, paths, http.clone()));
            app.manage(http);
            app.manage(Arc::clone(&local_model));
            tauri::async_runtime::spawn(async move { local_model.start_if_installed().await });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::watch_local_model,
            commands::download_local_model,
            commands::ask_text
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app, event| {
            // Tauri ends with process::exit, which skips destructors, so kill_on_drop never fires.
            if let RunEvent::Exit = event {
                let local_model = app.state::<Arc<LocalModel>>();
                tauri::async_runtime::block_on(local_model.shutdown());
            }
        });
}
```

- [ ] **Step 10: Run the tests and the linter**

Run: `pnpm -C app build && cargo test -p hey-dot && cargo clippy -p hey-dot --all-targets -- -D warnings`
Expected: `test result: ok. 7 passed; 0 failed`; clippy prints no warnings.

- [ ] **Step 11: See the app start llama-server**

Only if Qwen3-VL (4B on Apple Silicon 16 GB+, 2B otherwise) is already under `~/Library/Application Support/Hey Dot/models/`; otherwise Task 7 Step 1 covers this path after downloading through the panel.

Run: `pnpm -C app tauri dev`, then in another terminal `pgrep -lf llama-server` and `tail -n 5 ~/Library/Logs/Hey\ Dot/llama-server.log`. Quit the app with Cmd+Q, then `pgrep -x llama-server || echo "no llama-server running"`.
Expected: one `llama-server` with `-m ... --mmproj ... -c 8192 --host 127.0.0.1 --port <port> --no-ui --jinja` and no key in its arguments; the log ends with `listening on http://127.0.0.1:<port>`; after quitting, `no llama-server running`.

- [ ] **Step 12: Update the app doc**

In `app/AGENTS.md` replace the "Entry points" line with:

```markdown
Entry points: `src-tauri/src/main.rs` -> `hey_dot_lib::run()` in `src-tauri/src/lib.rs`; UI starts at `src/main.tsx`. IPC commands live in `src-tauri/src/commands.rs`.
```

and add these bullets after the build-script bullet from Task 4:

```markdown
- The bundled llama-server sits next to the app's executable (`Contents/MacOS/` in the bundle, `target/debug/` in `tauri dev`), which is how `lib.rs` finds it.
- `RunEvent::Exit` stops llama-server with `block_on`: Tauri ends with `process::exit`, which skips destructors, so `kill_on_drop` alone never fires.
- Paths the app owns: models in `~/Library/Application Support/Hey Dot/models/`, llama-server log in `~/Library/Logs/Hey Dot/llama-server.log`.
- Which local model runs is `pick_local_model(recommend(hardware))` until onboarding lets the user choose.
```

- [ ] **Step 13: Commit**

```bash
cargo fmt --all
git add app/src-tauri/Cargo.toml Cargo.lock app/src-tauri/src/local_model.rs
git commit -m "Keep the picked local model downloaded and its llama-server running"
git add app/src-tauri/src/commands.rs app/src-tauri/src/lib.rs app/AGENTS.md
git commit -m "Expose model status, download and typed questions as IPC commands"
```

---

### Task 6: A minimal chat panel

**Files:**
- Create: `app/src/chat/ipc.ts`
- Create: `app/src/chat/ChatPanel.tsx`
- Create: `app/src/chat/ChatPanel.test.tsx`
- Modify: `app/src/App.tsx`
- Modify: `app/src/App.test.tsx`
- Modify: `app/AGENTS.md`

**Interfaces:**
- Consumes: Task 5's three commands and their JSON shapes (listed in Task 5's Interfaces).
- Produces: `ipc.ts` exports `LocalModelPhase`, `LocalModelStatus`, `AnswerEvent`, `watchLocalModel(onStatus)`, `downloadLocalModel()`, `askText(question, onEvent)`; `ChatPanel` default export, rendered by `App`.

- [ ] **Step 1: Write the failing tests**

`@tauri-apps/api/mocks`' `mockIPC` hands `Channel` arguments to the handler as the real objects, so a test drives the panel by calling `channel.onmessage(...)`. vitest globals are off in this app, so each test file calls Testing Library's `cleanup()` itself.

`app/src/chat/ChatPanel.test.tsx`:

```tsx
import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import type { Channel } from "@tauri-apps/api/core";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { afterEach, expect, test } from "vitest";
import ChatPanel from "./ChatPanel";
import type { AnswerEvent, LocalModelPhase, LocalModelStatus } from "./ipc";

type AskHandler = (question: string, onEvent: Channel<AnswerEvent>) => Promise<void>;

let invokedCommands: string[] = [];
let statusChannel: Channel<LocalModelStatus> | undefined;

async function renderPanel(onAsk: AskHandler = async () => {}) {
  mockIPC((command, args) => {
    invokedCommands.push(command);
    const namedArgs = args as Record<string, unknown>;
    if (command === "watch_local_model") {
      statusChannel = namedArgs.onStatus as Channel<LocalModelStatus>;
    }
    if (command === "ask_text") {
      return onAsk(namedArgs.question as string, namedArgs.onEvent as Channel<AnswerEvent>);
    }
  });
  render(<ChatPanel />);
  await waitFor(() => expect(statusChannel).toBeDefined());
}

function sendPhase(phase: LocalModelPhase) {
  act(() => statusChannel!.onmessage({ modelName: "Qwen3-VL 4B", downloadBytes: 3_300_000_000, phase }));
}

function typeQuestion(text: string) {
  fireEvent.change(screen.getByLabelText("Question"), { target: { value: text } });
}

const askButton = () => screen.getByRole("button", { name: "Ask" }) as HTMLButtonElement;

afterEach(() => {
  cleanup();
  clearMocks();
  invokedCommands = [];
  statusChannel = undefined;
});

test("offers to download a missing model with its size", async () => {
  await renderPanel();
  sendPhase({ kind: "notInstalled" });
  expect(screen.getByText(/3\.3 GB/)).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "Download Qwen3-VL 4B" }));
  await waitFor(() => expect(invokedCommands).toContain("download_local_model"));
});

test("shows download progress as a percent", async () => {
  await renderPanel();
  sendPhase({ kind: "downloading", percent: 42 });
  expect(screen.getByText("Downloading Qwen3-VL 4B: 42%")).toBeTruthy();
});

test("a failed download shows the reason and can be retried", async () => {
  await renderPanel();
  sendPhase({ kind: "downloadFailed", reason: "server answered 503" });
  expect(screen.getByText(/server answered 503/)).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "Try again" }));
  await waitFor(() => expect(invokedCommands).toContain("download_local_model"));
});

test("a stopped server shows the reason and where its log is", async () => {
  await renderPanel();
  sendPhase({ kind: "down", reason: "llama-server stopped 4 times within a minute" });
  expect(screen.getByText(/stopped 4 times within a minute/)).toBeTruthy();
  expect(screen.getByText(/~\/Library\/Logs\/Hey Dot\/llama-server\.log/)).toBeTruthy();
});

test("ask stays disabled until the model is ready", async () => {
  await renderPanel();
  typeQuestion("What is the capital of France?");
  sendPhase({ kind: "starting" });
  expect(askButton().disabled).toBe(true);
  sendPhase({ kind: "ready" });
  expect(askButton().disabled).toBe(false);
});

test("ask stays disabled for a blank question", async () => {
  await renderPanel();
  sendPhase({ kind: "ready" });
  typeQuestion("   ");
  expect(askButton().disabled).toBe(true);
});

test("streams the answer under the question with an on-this-mac badge", async () => {
  await renderPanel(async (_question, onEvent) => {
    onEvent.onmessage({ event: "started", data: { leavesDevice: false, host: "127.0.0.1" } });
    onEvent.onmessage({ event: "delta", data: { text: "Par" } });
    onEvent.onmessage({ event: "delta", data: { text: "is" } });
  });
  sendPhase({ kind: "ready" });
  typeQuestion("What is the capital of France?");
  fireEvent.click(askButton());
  expect(await screen.findByText("Paris")).toBeTruthy();
  expect(screen.getByText("On this Mac")).toBeTruthy();
  expect(screen.getByText("What is the capital of France?")).toBeTruthy();
});

test("an answer that leaves the Mac names the host", async () => {
  await renderPanel(async (_question, onEvent) => {
    onEvent.onmessage({ event: "started", data: { leavesDevice: true, host: "api.openai.com" } });
  });
  sendPhase({ kind: "ready" });
  typeQuestion("Hi");
  fireEvent.click(askButton());
  expect(await screen.findByText("Sent to api.openai.com")).toBeTruthy();
});

test("a failed answer keeps the text so far and shows the error", async () => {
  await renderPanel(async (_question, onEvent) => {
    onEvent.onmessage({ event: "delta", data: { text: "Par" } });
    throw "could not reach the model server: the connection closed before the answer finished";
  });
  sendPhase({ kind: "ready" });
  typeQuestion("What is the capital of France?");
  fireEvent.click(askButton());
  expect((await screen.findByRole("alert")).textContent).toContain("connection closed");
  expect(screen.getByText("Par")).toBeTruthy();
  typeQuestion("Try again?");
  expect(askButton().disabled).toBe(false);
});

test("sending a question clears the box", async () => {
  await renderPanel();
  sendPhase({ kind: "ready" });
  typeQuestion("What is the capital of France?");
  fireEvent.click(askButton());
  expect((screen.getByLabelText("Question") as HTMLTextAreaElement).value).toBe("");
});

test("ask stays disabled while an answer is streaming", async () => {
  await renderPanel(() => new Promise(() => {}));
  sendPhase({ kind: "ready" });
  typeQuestion("What is the capital of France?");
  fireEvent.click(askButton());
  typeQuestion("And of Spain?");
  expect(askButton().disabled).toBe(true);
});
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `pnpm -C app test`
Expected: `ChatPanel.test.tsx` fails with `Failed to resolve import "./ChatPanel"`; `App.test.tsx` still passes.

- [ ] **Step 3: Implement the IPC boundary and the panel**

`app/src/chat/ipc.ts`:

```ts
// The typed boundary between the chat panel and the Rust commands in src-tauri/src/commands.rs.
// Must not hold UI state; the shapes here mirror the Rust serde output exactly.

import { Channel, invoke } from "@tauri-apps/api/core";

export type LocalModelPhase =
  | { kind: "notInstalled" }
  | { kind: "downloading"; percent: number }
  | { kind: "downloadFailed"; reason: string }
  | { kind: "starting" }
  | { kind: "ready" }
  | { kind: "down"; reason: string };

export type LocalModelStatus = {
  modelName: string;
  downloadBytes: number;
  phase: LocalModelPhase;
};

export type AnswerEvent =
  | { event: "started"; data: { leavesDevice: boolean; host: string } }
  | { event: "delta"; data: { text: string } };

export function watchLocalModel(onStatus: (status: LocalModelStatus) => void): Promise<void> {
  return invoke("watch_local_model", { onStatus: new Channel(onStatus) });
}

export function downloadLocalModel(): Promise<void> {
  return invoke("download_local_model");
}

/** Rejects with the error text when the answer fails part way. */
export function askText(question: string, onEvent: (event: AnswerEvent) => void): Promise<void> {
  return invoke("ask_text", { question, onEvent: new Channel(onEvent) });
}
```

`app/src/chat/ChatPanel.tsx`:

```tsx
// The first chat surface: the local model's state, one typed question and its streamed answer.
// Must not call invoke directly; every backend call goes through ./ipc.

import { useEffect, useState } from "react";
import { askText, downloadLocalModel, watchLocalModel, type LocalModelStatus } from "./ipc";

const SERVER_LOG_PATH = "~/Library/Logs/Hey Dot/llama-server.log";

type Answer = {
  question: string;
  text: string;
  badge: string | null;
  error: string | null;
};

function describeModel(status: LocalModelStatus | null) {
  if (status === null) {
    return <p>Checking the local model...</p>;
  }
  const { modelName, phase } = status;
  switch (phase.kind) {
    case "notInstalled":
      return (
        <p>
          {modelName} is not downloaded yet ({(status.downloadBytes / 1e9).toFixed(1)} GB).{" "}
          <button onClick={() => void downloadLocalModel()}>Download {modelName}</button>
        </p>
      );
    case "downloading":
      return (
        <p>
          Downloading {modelName}: {phase.percent}%
        </p>
      );
    case "downloadFailed":
      return (
        <p>
          The download failed: {phase.reason}{" "}
          <button onClick={() => void downloadLocalModel()}>Try again</button>
        </p>
      );
    case "starting":
      return <p>Starting {modelName}...</p>;
    case "ready":
      return <p>{modelName} is ready.</p>;
    case "down":
      return (
        <p>
          {modelName} stopped: {phase.reason}. Details are in {SERVER_LOG_PATH}
        </p>
      );
  }
}

export default function ChatPanel() {
  const [status, setStatus] = useState<LocalModelStatus | null>(null);
  const [question, setQuestion] = useState("");
  const [answer, setAnswer] = useState<Answer | null>(null);
  const [asking, setAsking] = useState(false);

  useEffect(() => {
    void watchLocalModel(setStatus);
  }, []);

  async function ask() {
    setAsking(true);
    setQuestion("");
    setAnswer({ question, text: "", badge: null, error: null });
    try {
      await askText(question, (event) =>
        setAnswer((current) => {
          if (current === null) return current;
          if (event.event === "started") {
            const badge = event.data.leavesDevice ? `Sent to ${event.data.host}` : "On this Mac";
            return { ...current, badge };
          }
          return { ...current, text: current.text + event.data.text };
        }),
      );
    } catch (error) {
      setAnswer((current) => current && { ...current, error: String(error) });
    } finally {
      setAsking(false);
    }
  }

  const canAsk = status?.phase.kind === "ready" && !asking && question.trim() !== "";

  return (
    <section>
      {describeModel(status)}
      <label>
        Question
        <textarea value={question} onChange={(event) => setQuestion(event.target.value)} />
      </label>
      <button disabled={!canAsk} onClick={() => void ask()}>
        Ask
      </button>
      {answer && (
        <article>
          <p>{answer.question}</p>
          {answer.badge && <p>{answer.badge}</p>}
          <p style={{ whiteSpace: "pre-wrap" }}>{answer.text}</p>
          {answer.error && <p role="alert">{answer.error}</p>}
        </article>
      )}
    </section>
  );
}
```

- [ ] **Step 4: Run the panel tests to verify they pass**

Run: `pnpm -C app test -- ChatPanel`
Expected: `11 passed`.

- [ ] **Step 5: Render the panel in the app**

`app/src/App.tsx`:

```tsx
import ChatPanel from "./chat/ChatPanel";

function App() {
  return (
    <main>
      <h1>Hey Dot</h1>
      <ChatPanel />
    </main>
  );
}

export default App;
```

`App` now calls `watch_local_model` on mount, so its test needs an IPC mock. `app/src/App.test.tsx`:

```tsx
import { render, screen } from "@testing-library/react";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { afterEach, beforeEach, expect, test } from "vitest";
import App from "./App";

beforeEach(() => mockIPC(() => {}));
afterEach(() => clearMocks());

test("renders the app name", () => {
  render(<App />);
  expect(screen.getByRole("heading", { name: "Hey Dot" })).toBeTruthy();
});
```

- [ ] **Step 6: Run the whole UI check**

Run: `pnpm -C app test && pnpm -C app build`
Expected: `Test Files  2 passed`, `Tests  12 passed`; `tsc` prints nothing and `vite build` finishes.

- [ ] **Step 7: Update the app doc**

In `app/AGENTS.md` change the end of the "Entry points" line to:

```markdown
IPC commands live in `src-tauri/src/commands.rs` and their TypeScript mirror in `src/chat/ipc.ts`.
```

and add this bullet after the `RunEvent::Exit` bullet:

```markdown
- The `AnswerEvent` and `LocalModelStatus` serde shapes are a contract with `src/chat/ipc.ts`; the serialization tests in `commands.rs` and `local_model.rs` pin them.
```

- [ ] **Step 8: Commit**

```bash
git add app/src/chat app/src/App.tsx app/src/App.test.tsx app/AGENTS.md
git commit -m "Add a minimal chat panel: model status, one typed question, streamed answer"
```

---

### Task 7: Prove it end to end with the real model

**Files:**
- Modify: `crates/dot-runtime/Cargo.toml` (dev-dependencies)
- Create: `crates/dot-runtime/tests/real_model.rs`
- Modify: `crates/dot-runtime/AGENTS.md`, `crates/dot-models/AGENTS.md`
- Modify: `Cargo.lock`

**Interfaces:**
- Consumes: everything above.
- Produces: nothing new; this is the proof.

- [ ] **Step 1: Download the model and ask through the app**

Run: `pnpm -C app tauri dev`. In the window, click "Download Qwen3-VL 4B" (2B on an Intel or 8 GB Mac), wait for "Qwen3-VL 4B is ready.", type "What is the capital of France? Answer in one word." and click Ask.
Expected: the percent climbs to 100 (measured 2:16 for about 3.3 GB), the status passes through "Starting" to "ready" (measured 17.3 s on the first load, 1.3 s warm), the answer streams "Paris" under the question with the "On this Mac" badge. Quit with Cmd+Q; `pgrep -x llama-server || echo "no llama-server running"` prints `no llama-server running`.

- [ ] **Step 2: Write the real-model test**

Add to `[dev-dependencies]` in `crates/dot-runtime/Cargo.toml`:

```toml
dot-models = { path = "../dot-models" }
dot-providers = { path = "../dot-providers" }
futures-util = "0.3.34"
```

`crates/dot-runtime/tests/real_model.rs`:

```rust
//! Proves the pinned llama-server runs the real Qwen3-VL 4B GGUF and answers through dot-providers.
//! Ignored by default: it needs `scripts/build-llama-server.sh` run and the model downloaded
//! (the app's Download button puts it where this test looks).

use std::path::PathBuf;
use std::time::{Duration, Instant};

use dot_models::{QWEN3_VL_4B, installed_chat_model};
use dot_providers::{ChatConfig, ChatMessage, ChatRole, stream_chat};
use dot_runtime::{Runtime, RuntimeConfig, RuntimeState};
use futures_util::StreamExt;

const MODEL_LOAD_TIMEOUT: Duration = Duration::from_secs(120);

#[tokio::test]
#[ignore = "needs the built llama-server and the downloaded Qwen3-VL 4B model"]
async fn the_real_4b_model_answers_a_typed_question() {
    let models_dir = std::env::home_dir()
        .unwrap()
        .join("Library/Application Support/Hey Dot/models");
    let files = installed_chat_model(&QWEN3_VL_4B, &models_dir)
        .expect("download Qwen3-VL 4B with the app first");
    let log_folder = tempfile::tempdir().unwrap();
    let log_file = log_folder.path().join("llama-server.log");
    let started_at = Instant::now();
    let runtime = Runtime::start(RuntimeConfig {
        server_binary: PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../app/src-tauri/binaries/llama-server-aarch64-apple-darwin"),
        model_file: files.weights,
        mmproj_file: files.projector,
        context_tokens: 8192,
        log_file: log_file.clone(),
    });

    let mut state = runtime.watch_state();
    let settled = tokio::time::timeout(
        MODEL_LOAD_TIMEOUT,
        state.wait_for(|state| *state != RuntimeState::Starting),
    )
    .await
    .expect("llama-server did not load the model within two minutes")
    .unwrap()
    .clone();
    let RuntimeState::Ready { base_url } = settled else {
        panic!(
            "{settled:?}\n{}",
            std::fs::read_to_string(&log_file).unwrap_or_default()
        );
    };
    eprintln!("ready after {:?}", started_at.elapsed());

    let config = ChatConfig {
        base_url,
        api_key: runtime.api_key().to_owned(),
        model: QWEN3_VL_4B.id.to_owned(),
    };
    let question = [ChatMessage {
        role: ChatRole::User,
        text: "What is the capital of France? Answer in one word.".to_owned(),
        jpeg_image: None,
    }];
    let asked_at = Instant::now();
    let http = reqwest::Client::new();
    let mut answer_stream = std::pin::pin!(stream_chat(&http, &config, &question));
    let mut answer = String::new();
    while let Some(delta) = answer_stream.next().await {
        if answer.is_empty() {
            eprintln!("first text after {:?}", asked_at.elapsed());
        }
        answer.push_str(&delta.unwrap());
    }
    eprintln!("whole answer after {:?}: {answer:?}", asked_at.elapsed());
    runtime.stop().await;

    assert!(answer.contains("Paris"), "{answer}");
}
```

- [ ] **Step 3: Run it**

Run: `cargo test -p dot-runtime --test real_model -- --ignored --nocapture`
Expected: `1 passed`, with timings printed. Measured on an Apple Silicon Mac on 2026-09-23: ready after 17.27 s cold and 1.26 s warm, first text after 108 to 150 ms, whole answer after 141 to 178 ms. llama-server's log (the test prints it only on failure) reports `n_slots = 4`, `n_ctx_slot = 8192`. Without `-- --ignored` it reports `1 ignored`.

- [ ] **Step 4: Update the module docs**

In `crates/dot-runtime/AGENTS.md` add after the fake-server bullet:

```markdown
- `tests/real_model.rs` is ignored: it needs `scripts/build-llama-server.sh` run and Qwen3-VL 4B downloaded to `~/Library/Application Support/Hey Dot/models`. It is the proof that the pinned llama-server runs that GGUF.
```

In `crates/dot-models/AGENTS.md` replace the "Qwen3-VL GGUF compatibility" bullet with:

```markdown
- Qwen3-VL GGUF compatibility with the pinned llama-server is proven by `crates/dot-runtime/tests/real_model.rs` (ignored), not here.
```

and the "Called by" line with:

```markdown
Called by: `app/src-tauri` (`local_model.rs`) and `crates/dot-runtime`'s ignored real-model test.
```

- [ ] **Step 5: Verify the whole workspace**

```bash
pnpm -C app build
pnpm -C app test
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Expected: vitest `12 passed`; fmt prints nothing; no clippy warnings; `cargo test --workspace` passes every target: dot-models catalog `4 passed; 1 ignored`, download `8 passed; 1 ignored`, installed `4 passed`, recommend `6 passed`; dot-providers leaves_device `2`, stream_chat `14`; dot-runtime runtime `10`, real_model `1 ignored`; dot-settings api_keys `3`, settings_file `6`; hey_dot_lib `7`.

- [ ] **Step 6: Smoke-test the bundled app**

Run: `pnpm -C app tauri build --debug --bundles app && open "target/debug/bundle/macos/Hey Dot.app"`, wait for the panel to say ready, then `pgrep -lf llama-server`, then `osascript -e 'tell application id "com.shub3am.heydot" to quit'` and `pgrep -x llama-server || echo "no llama-server running"; pgrep -x hey-dot || echo "no hey-dot running"`.
Expected: llama-server runs from `Hey Dot.app/Contents/MacOS/llama-server` with no key in its arguments; after quitting, both lines print "no ... running".

- [ ] **Step 7: Commit**

```bash
git add crates/dot-runtime/Cargo.toml crates/dot-runtime/tests/real_model.rs Cargo.lock crates/dot-runtime/AGENTS.md crates/dot-models/AGENTS.md
git commit -m "Prove the pinned llama-server answers with the real Qwen3-VL 4B model"
```

Then show the user the Step 3, 5 and 6 output and wait before `git push`.
