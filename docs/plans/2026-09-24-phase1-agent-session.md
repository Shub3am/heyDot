# Phase 1 step 5: dot-agent, a session that remembers turns and answers follow-ups Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A question typed in the chat panel is answered with the earlier turns of the conversation, so "How many people live there?" works after "What city is this?". A new question while an answer streams stops that answer and asks the new one. "New chat" forgets the conversation, and ten idle minutes do the same.

**Architecture:** A new crate `dot-agent` owns the conversation. `History` remembers the turns and applies the rules for forgetting them: ten turns, ten idle minutes, New chat. `build_messages` turns them into one request: system prompt, earlier turns as text, and only the new question carries its screenshot. `Session::ask` streams `AgentEvent`s. It holds the history lock for one answer and ends early when a `CancellationToken` fires. Each new question or `new_chat` cancels the token of the running answer. The app keeps one managed `Session`: `ask_text` asks through it, and a new `new_chat` command resets it. The panel shows the whole conversation, keeps Ask enabled while an answer streams, and has a "New chat" button. `dot-providers` gains `ProviderError::ContextTooLong`, so a chat that outgrows the model's 8192-token context tells the user to start a new chat.

**Tech Stack:** Rust 1.98 edition 2024, Tauri 2.11.6; `dot-agent` uses `dot-providers` (path), `async-stream` 0.3.6, `futures-util` 0.3.34, `reqwest` 0.13.5, `tokio` 1.53.1 (`sync`), `tokio-util` 0.7.19 (its `sync` module needs no feature). Tests use `wiremock` 0.6.5, `serde_json`, tokio `net`/`io-util`/`time`. All versions are the ones already in `Cargo.lock`. I checked every API below against the 0.7.19, 0.3.6 and 0.6.5 sources on 2026-09-24. I also measured every model behavior below on the real Qwen3-VL 4B GGUF behind the pinned llama-server on this Mac.

**Spec:** `docs/specs/2026-09-23-phase0-phase1-design.md` (sections "dot-agent: one conversation turn at a time", "Chat panel", "Error states the user sees", "Testing", build order step 5).

## Global Constraints

- Crate name is final: `dot-agent`. It joins the workspace through the existing `members = ["app/src-tauri", "crates/*"]`, so the root `Cargo.toml` does not change.
- "`Session` holds turns. `ask(UserInput { text, screenshot }) -> Stream<AgentEvent>` where events are `Thinking`, `Delta(text)`, `Done`, `Error(kind)`." The spec's public items are indicative. Ruling: `Done` is the end of the stream, `Error(kind)` is an `Err(ProviderError)` item, and `Thinking` carries `forgot_earlier_turns`, so the UI learns about an idle reset.
- "Context policy: system prompt + last 10 turns; only the newest turn carries its image, older turns keep text with `[screenshot from earlier turn]`. Session resets after 10 min idle or on "New chat"." The ten turns include the new question, so a request carries at most nine earlier turns.
- "One turn in flight; a new question cancels the running one (and its speech)." Speech arrives in step 7; this step cancels the answer.
- UI -> Rust commands per the spec: `ask_text`, `new_chat`. `stop_speaking` arrives with speech in step 7. Markdown and the copy button belong to step 8.
- Screenshots of earlier turns are never stored: `History` keeps only whether a turn had one.
- Every source file starts with a header that says why it exists and what it must not do. It never lists functions.
- Every crate has an `AGENTS.md` covering what it owns, what it must not know about, its entry points, its invariants and gotchas, and who calls it.
- Shell setup for every command: `export PATH="$HOME/.cargo/bin:$PATH"`, working directory the repo root.
- `pnpm -C app build` and `scripts/build-llama-server.sh` must have run before any cargo command that builds `hey-dot`.
- Before Task 1: `git tag before-phase1-step5` (local only). This is the named point to return to.
- All work happens on `main`.
- No em dashes in any file.
- Commits: one logical change each, no co-author lines. Nothing is pushed until the user has seen the test output.

## Verified facts the tasks rely on

- **System prompt:** Qwen3-VL 4B behind llama-server obeys a system message sent as a `[{"type":"text"}]` parts array, which is how `dot-providers` sends every message.
- **Follow-ups:** the model answers from history sent as plain user/assistant messages. It was sent the question "My favorite color is teal...\n[screenshot from earlier turn]" with the answer "OK BANANA", then asked a follow-up, and it answered "Teal". The note after the question did not confuse it.
- **Image cost:** a 1600x900 JPEG costs about 1,410 prompt tokens: 1,479 with one earlier turn, 65 without the image. The app's context is 8192 tokens (`CONTEXT_TOKENS` in `app/src-tauri/src/local_model.rs`). So only the newest image fits comfortably, which is why the spec drops older ones.
- **Context overflow:** llama-server answers a request longer than its context with HTTP 400 and this body: `{"error":{"code":400,"message":"request (12012 tokens) exceeds the available context size (8192 tokens), try increasing it","type":"exceed_context_size_error","n_prompt_tokens":12012,"n_ctx":8192}}`. Today that surfaces as `Other { status: Some(400), body }`, so the panel shows raw JSON.
- **`run_until_cancelled`:** `CancellationToken::run_until_cancelled(fut) -> Option<F::Output>` returns `None` at once if the token is already cancelled. It is biased toward the future, so a delta that is already ready is not lost. `CancellationToken` implements `Default` and `Debug`.
- **Mutex order:** `tokio::sync::Mutex` hands out the lock in FIFO order, so questions are answered in the order they reached the session.
- **`try_stream!`:** `async_stream::try_stream!` supports `return;` and `Err(error)?;` inside its body; `dot-providers/src/stream_chat.rs` already uses both.
- **Tauri state:** a Tauri async command that borrows `State` must return `Result`. `ask_text` already does, and `new_chat` returns `Result<(), String>`.
- **wiremock:** `MockServer::received_requests().await` returns the requests in order, and `Request::body_json::<serde_json::Value>()` parses one. The `dot-providers` tests already read request bodies this way.

## Review Focus

1. **A long chat outgrows the 8192-token context.** Several long answers push a later question past 8192 tokens. Expected: the panel says "This chat is too long for the model. Click New chat to start over.", not raw JSON, and New chat recovers. Pinned by the `ContextTooLong` mapping test (Task 4), the `describe_answer_error` test (Task 5), and the real-model overflow test (Task 7).
2. **A new question while an answer streams.** Expected: the old answer stops at once and keeps the text it had, the new question is answered, and the new request carries the old turn with its partial answer. Pinned by `a_new_question_stops_the_running_answer_and_keeps_its_text_so_far` (Task 3) and the panel test "asking while an answer streams sends the new question and keeps the earlier one" (Task 6).
3. **New chat while an answer streams.** Expected: the answer stops, no late text appears in the empty panel, and the next question carries no earlier turn. Pinned by `new_chat_stops_the_running_answer` (Task 3) and the panel test "an answer still streaming when New chat is clicked does not come back" (Task 6).
4. **The user comes back after ten or more idle minutes.** Expected: Hey Dot has forgotten the earlier turns, and the panel drops them too instead of implying they still count. Pinned by the idle tests in `history.rs` (Task 2) and the panel test "a question Hey Dot answers after ten idle minutes hides the turns it forgot" (Task 6).
5. **An answer fails part way (server killed, network error).** Expected: the error shows under that question, and the next question does not carry the failed half-answer. Pinned by `a_failed_answer_is_left_out_of_the_next_question` (Task 3).

---

### Task 1: dot-agent crate and the request built from earlier turns

**Files:**
- Create: `crates/dot-agent/Cargo.toml`
- Create: `crates/dot-agent/src/lib.rs`
- Create: `crates/dot-agent/src/context.rs`
- Create: `crates/dot-agent/AGENTS.md`
- Modify: `AGENTS.md` (Modules list)

**Interfaces:**
- Produces:
  - `pub struct UserInput { pub text: String, pub jpeg_screenshot: Option<Vec<u8>> }`, re-exported from `lib.rs`.
  - `pub(crate) struct Turn { question: String, had_screenshot: bool, answer: String }`.
  - `pub(crate) fn build_messages(earlier: &[Turn], input: UserInput) -> Vec<ChatMessage>`.
  - Task 2 uses the last two.

- [ ] **Step 1: Tag and scaffold**

```bash
git tag before-phase1-step5
```

`crates/dot-agent/Cargo.toml`:

```toml
[package]
name = "dot-agent"
version = "0.1.0"
edition = "2024"
license = "Apache-2.0"
publish = false

[dependencies]
dot-providers = { path = "../dot-providers" }
```

`crates/dot-agent/src/lib.rs`:

```rust
//! One conversation with the model: the turns it remembers and the one answer in flight.
//! Must not capture the screen, speak, or know about Tauri, the UI or which model serves the answer.

mod context;

pub use context::UserInput;
```

- [ ] **Step 2: Write the failing tests**

`crates/dot-agent/src/context.rs`:

```rust
//! What the model is sent for one question: the system prompt, earlier turns as text, the new question with its screenshot.
//! Must not decide which turns are remembered or when they are forgotten; history.rs does.

use dot_providers::{ChatMessage, ChatRole};

/// A typed or spoken question and the screenshot taken when it was asked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UserInput {
    pub text: String,
    pub jpeg_screenshot: Option<Vec<u8>>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn earlier_turn(question: &str, had_screenshot: bool, answer: &str) -> Turn {
        Turn {
            question: question.to_owned(),
            had_screenshot,
            answer: answer.to_owned(),
        }
    }

    fn with_screenshot(text: &str) -> UserInput {
        UserInput {
            text: text.to_owned(),
            jpeg_screenshot: Some(vec![0xFF, 0xD8]),
        }
    }

    #[test]
    fn the_system_prompt_comes_first() {
        let messages = build_messages(&[], with_screenshot("What is on my screen?"));

        assert_eq!(messages[0].role, ChatRole::System);
        assert_eq!(messages[0].text, SYSTEM_PROMPT);
        assert_eq!(messages.len(), 2);
    }

    #[test]
    fn only_the_new_question_carries_its_screenshot() {
        let earlier = [earlier_turn("What city is this?", true, "Paris")];

        let messages = build_messages(&earlier, with_screenshot("How many people live there?"));

        let roles: Vec<ChatRole> = messages.iter().map(|message| message.role).collect();
        assert_eq!(
            roles,
            [ChatRole::System, ChatRole::User, ChatRole::Assistant, ChatRole::User]
        );
        assert_eq!(messages[1].jpeg_image, None);
        assert_eq!(messages[2].text, "Paris");
        assert_eq!(messages[3].text, "How many people live there?");
        assert_eq!(messages[3].jpeg_image, Some(vec![0xFF, 0xD8]));
    }

    #[test]
    fn an_earlier_screenshot_leaves_a_note_after_its_question() {
        let earlier = [earlier_turn("What city is this?", true, "Paris")];

        let messages = build_messages(&earlier, with_screenshot("And its population?"));

        assert_eq!(
            messages[1].text,
            "What city is this?\n[screenshot from earlier turn]"
        );
    }

    #[test]
    fn an_earlier_question_without_a_screenshot_has_no_note() {
        let earlier = [earlier_turn("What is the capital of France?", false, "Paris")];

        let messages = build_messages(&earlier, with_screenshot("And of Spain?"));

        assert_eq!(messages[1].text, "What is the capital of France?");
    }
}
```

`lib.rs` already declares `mod context;` from Step 1.

- [ ] **Step 3: Run the tests to verify they fail**

Run: `cargo test -p dot-agent`
Expected: FAIL to compile: `cannot find type Turn`, `cannot find function build_messages`, `cannot find value SYSTEM_PROMPT`.

- [ ] **Step 4: Write the implementation**

Insert into `crates/dot-agent/src/context.rs`, between the `UserInput` struct and the tests:

```rust
pub(crate) const SYSTEM_PROMPT: &str = "You are Hey Dot, an assistant on the user's Mac. When a screenshot is attached, it shows the user's screen at the moment they asked; use it to answer. Answer briefly and directly.";

const EARLIER_SCREENSHOT_NOTE: &str = "[screenshot from earlier turn]";

/// A question already sent. Its screenshot is never kept, only whether it had one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Turn {
    pub(crate) question: String,
    pub(crate) had_screenshot: bool,
    pub(crate) answer: String,
}

pub(crate) fn build_messages(earlier: &[Turn], input: UserInput) -> Vec<ChatMessage> {
    let mut messages = vec![ChatMessage {
        role: ChatRole::System,
        text: SYSTEM_PROMPT.to_owned(),
        jpeg_image: None,
    }];
    for turn in earlier {
        // The note follows the question because a request puts a message's text before its image.
        let question = if turn.had_screenshot {
            format!("{}\n{EARLIER_SCREENSHOT_NOTE}", turn.question)
        } else {
            turn.question.clone()
        };
        messages.push(ChatMessage {
            role: ChatRole::User,
            text: question,
            jpeg_image: None,
        });
        messages.push(ChatMessage {
            role: ChatRole::Assistant,
            text: turn.answer.clone(),
            jpeg_image: None,
        });
    }
    messages.push(ChatMessage {
        role: ChatRole::User,
        text: input.text,
        jpeg_image: input.jpeg_screenshot,
    });
    messages
}
```

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test -p dot-agent && cargo clippy -p dot-agent --all-targets -- -D warnings`
Expected: 4 passed. Clippy may warn `Turn`/`build_messages` are unused outside tests. If so, the `dead_code` warnings disappear in Task 2. For this commit only, run clippy without `-D warnings` and note it in the ledger.

- [ ] **Step 6: Module docs**

`crates/dot-agent/AGENTS.md`:

```markdown
# dot-agent

Owns: one conversation with the model. It remembers the turns and builds each request from them: the system prompt, up to nine earlier turns as text, and the new question with its screenshot. It keeps one answer in flight: a newer question or New chat cancels the running answer.

Must not know about: screen capture, speech, the UI, Tauri, llama-server, or which provider is behind the `ChatConfig`. It never retries.

Entry points:
- `Session::ask(&client, &config, UserInput)`: a stream of `AgentEvent`s. It ends when the answer is complete or cancelled, or after an `Err(ProviderError)` item.
- `Session::new_chat()`.

Invariants and gotchas:
- The spec's `Done` is the end of the stream, and its `Error(kind)` is an `Err(ProviderError)` item.
- "Last 10 turns" counts the new question, so a request carries at most nine earlier turns.
- Only the new question carries an image. An earlier question that had one ends with "[screenshot from earlier turn]" on its own line. Screenshots are never stored.
- Ten idle minutes, counted from the last question, forget every turn. The next `Thinking` then says `forgot_earlier_turns: true`, so the UI can drop them too.
- A cancelled answer stays in history with its text so far. A failed answer is discarded. A question with no answer text is forgotten when the next question starts.
- The history lock is held for a whole answer, so `new_chat` waits until the running answer notices its cancellation. A consumer that stops polling a stream without dropping it blocks every later question.
- Calling `ask` cancels the running answer at once, before the returned stream is polled. A question that is superseded before it gets the lock yields nothing at all.
- A context overflow is not trimmed: the request fails with `ContextTooLong`, and the user starts a new chat.

Called by: `app/src-tauri` (the `ask_text` and `new_chat` commands) and `crates/dot-runtime`'s ignored real-model test.
```

Root `AGENTS.md`, after the `crates/dot-providers/` line:

```markdown
- `crates/dot-agent/`: one conversation: remembered turns, the request built from them, one answer in flight. See `crates/dot-agent/AGENTS.md`.
```

- [ ] **Step 7: Commit**

```bash
git add crates/dot-agent AGENTS.md Cargo.lock
git commit -m "dot-agent: build a request from earlier turns, only the newest with its screenshot"
```

---

### Task 2: remembered turns, ten at most, forgotten after ten idle minutes

**Files:**
- Create: `crates/dot-agent/src/history.rs`
- Modify: `crates/dot-agent/src/lib.rs` (add `mod history;`)

**Interfaces:**
- Consumes: `Turn`, `UserInput`, `build_messages` from Task 1.
- Produces, used by Task 3:
  - `#[derive(Debug, Default)] pub(crate) struct History`.
  - `pub(crate) struct StartedTurn { pub(crate) messages: Vec<ChatMessage>, pub(crate) forgot_earlier_turns: bool }`.
  - `History::start_turn(&mut self, input: UserInput, now: Instant) -> StartedTurn`.
  - `History::append_to_answer(&mut self, text: &str)`.
  - `History::discard_running_turn(&mut self)`.
  - `History::clear(&mut self)`.

- [ ] **Step 1: Write the failing tests**

`crates/dot-agent/src/history.rs`:

```rust
//! The turns a session remembers and when it forgets them: past ten turns, after ten idle minutes, on New chat.
//! Must not build requests or talk to the model; context.rs and session.rs do.

use std::time::{Duration, Instant};

use dot_providers::ChatMessage;

use crate::context::{Turn, UserInput, build_messages};

#[cfg(test)]
mod tests {
    use super::*;

    fn question(text: &str) -> UserInput {
        UserInput {
            text: text.to_owned(),
            jpeg_screenshot: None,
        }
    }

    fn ask_and_answer(history: &mut History, text: &str, now: Instant) -> StartedTurn {
        let started = history.start_turn(question(text), now);
        history.append_to_answer(&format!("answer to {text}"));
        started
    }

    #[test]
    fn keeps_at_most_ten_turns_including_the_new_question() {
        let mut history = History::default();
        let start = Instant::now();
        for number in 1..=11 {
            ask_and_answer(&mut history, &format!("question {number}"), start + Duration::from_secs(number));
        }

        let started = history.start_turn(question("question 12"), start + Duration::from_secs(12));

        assert_eq!(started.messages.len(), 1 + 9 * 2 + 1);
        assert_eq!(started.messages[1].text, "question 3");
        assert_eq!(started.messages[19].text, "question 12");
    }

    #[test]
    fn forgets_earlier_turns_after_ten_idle_minutes() {
        let mut history = History::default();
        let start = Instant::now();
        ask_and_answer(&mut history, "What city is this?", start);

        let started = history.start_turn(question("How many people live there?"), start + Duration::from_secs(600));

        assert_eq!(started.messages.len(), 2);
        assert!(started.forgot_earlier_turns);
    }

    #[test]
    fn keeps_earlier_turns_within_ten_idle_minutes() {
        let mut history = History::default();
        let start = Instant::now();
        ask_and_answer(&mut history, "What city is this?", start);

        let started = history.start_turn(question("How many people live there?"), start + Duration::from_secs(599));

        assert_eq!(started.messages.len(), 4);
        assert!(!started.forgot_earlier_turns);
    }

    #[test]
    fn idle_minutes_count_from_the_last_question() {
        let mut history = History::default();
        let start = Instant::now();
        ask_and_answer(&mut history, "question 1", start);
        ask_and_answer(&mut history, "question 2", start + Duration::from_secs(500));

        let started = history.start_turn(question("question 3"), start + Duration::from_secs(1000));

        assert_eq!(started.messages.len(), 6);
    }

    #[test]
    fn the_first_question_forgot_nothing() {
        let started = History::default().start_turn(question("Hi"), Instant::now());

        assert!(!started.forgot_earlier_turns);
    }

    #[test]
    fn a_discarded_turn_is_left_out_of_the_next_question() {
        let mut history = History::default();
        let start = Instant::now();
        history.start_turn(question("What city is this?"), start);
        history.append_to_answer("Par");
        history.discard_running_turn();

        let started = history.start_turn(question("Try again?"), start + Duration::from_secs(1));

        assert_eq!(started.messages.len(), 2);
    }

    #[test]
    fn a_question_left_without_any_answer_text_is_forgotten() {
        let mut history = History::default();
        let start = Instant::now();
        history.start_turn(question("What city is this?"), start);

        let started = history.start_turn(question("And now?"), start + Duration::from_secs(1));

        assert_eq!(started.messages.len(), 2);
    }

    #[test]
    fn clear_forgets_every_turn() {
        let mut history = History::default();
        let start = Instant::now();
        ask_and_answer(&mut history, "What city is this?", start);
        history.clear();

        let started = history.start_turn(question("Hi"), start + Duration::from_secs(1));

        assert_eq!(started.messages.len(), 2);
        assert!(!started.forgot_earlier_turns);
    }
}
```

`crates/dot-agent/src/lib.rs` gains `mod history;` under `mod context;`.

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p dot-agent`
Expected: FAIL to compile: `cannot find type History`, `cannot find type StartedTurn`.

- [ ] **Step 3: Write the implementation**

Insert into `history.rs` between the `use` lines and the tests:

```rust
/// Counts the new question, so a request carries at most nine earlier turns.
const MAX_TURNS: usize = 10;
const IDLE_RESET: Duration = Duration::from_secs(10 * 60);

#[derive(Debug, Default)]
pub(crate) struct History {
    turns: Vec<Turn>,
    last_asked: Option<Instant>,
}

pub(crate) struct StartedTurn {
    pub(crate) messages: Vec<ChatMessage>,
    pub(crate) forgot_earlier_turns: bool,
}

impl History {
    /// Remembers the new question as the running turn and returns the request that asks it.
    pub(crate) fn start_turn(&mut self, input: UserInput, now: Instant) -> StartedTurn {
        let idle = self
            .last_asked
            .is_some_and(|last_asked| now.duration_since(last_asked) >= IDLE_RESET);
        let forgot_earlier_turns = idle && !self.turns.is_empty();
        if idle {
            self.turns.clear();
        }
        self.last_asked = Some(now);
        self.turns.retain(|turn| !turn.answer.is_empty());
        let excess = self.turns.len().saturating_sub(MAX_TURNS - 1);
        self.turns.drain(..excess);
        let running_turn = Turn {
            question: input.text.clone(),
            had_screenshot: input.jpeg_screenshot.is_some(),
            answer: String::new(),
        };
        let messages = build_messages(&self.turns, input);
        self.turns.push(running_turn);
        StartedTurn {
            messages,
            forgot_earlier_turns,
        }
    }

    pub(crate) fn append_to_answer(&mut self, text: &str) {
        self.turns
            .last_mut()
            .expect("an answer arrives only while its turn is running")
            .answer
            .push_str(text);
    }

    pub(crate) fn discard_running_turn(&mut self) {
        self.turns.pop();
    }

    pub(crate) fn clear(&mut self) {
        *self = Self::default();
    }
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p dot-agent`
Expected: 12 passed (4 context, 8 history).

- [ ] **Step 5: Commit**

```bash
git add crates/dot-agent/src
git commit -m "dot-agent: remember ten turns and forget them after ten idle minutes"
```

---

### Task 3: Session, one answer in flight

**Files:**
- Create: `crates/dot-agent/src/session.rs`
- Create: `crates/dot-agent/tests/session.rs`
- Modify: `crates/dot-agent/src/lib.rs`
- Modify: `crates/dot-agent/Cargo.toml`

**Interfaces:**
- Consumes: `History`, `StartedTurn` from Task 2; `stream_chat`, `ChatConfig`, `ProviderError` from dot-providers.
- Produces, used by Tasks 5 and 7:
  - `pub enum AgentEvent { Thinking { forgot_earlier_turns: bool }, Delta(String) }`, which derives `Debug, Clone, PartialEq, Eq`.
  - `#[derive(Default)] pub struct Session`.
  - `Session::ask<'a>(&'a self, http: &'a reqwest::Client, config: &'a ChatConfig, input: UserInput) -> impl Stream<Item = Result<AgentEvent, ProviderError>> + 'a`.
  - `Session::new_chat(&self)` (async).
  - `lib.rs` re-exports `Session`, `AgentEvent` and `UserInput`.

- [ ] **Step 1: Dependencies**

`crates/dot-agent/Cargo.toml` becomes:

```toml
[package]
name = "dot-agent"
version = "0.1.0"
edition = "2024"
license = "Apache-2.0"
publish = false

[dependencies]
async-stream = "0.3.6"
dot-providers = { path = "../dot-providers" }
futures-util = "0.3.34"
reqwest = "0.13.5"
tokio = { version = "1.53.1", features = ["sync"] }
tokio-util = "0.7.19"

[dev-dependencies]
serde_json = "1.0.151"
tokio = { version = "1.53.1", features = ["io-util", "macros", "net", "rt-multi-thread", "time"] }
wiremock = "0.6.5"
```

- [ ] **Step 2: Write the failing tests**

`crates/dot-agent/tests/session.rs`:

```rust
use std::time::Duration;

use dot_agent::{AgentEvent, Session, UserInput};
use dot_providers::{ChatConfig, ProviderError};
use futures_util::StreamExt;
use serde_json::json;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn config_for(base_url: String) -> ChatConfig {
    ChatConfig {
        base_url: format!("{base_url}/v1").parse().unwrap(),
        api_key: "test-key".to_owned(),
        model: "test-model".to_owned(),
    }
}

fn question(text: &str) -> UserInput {
    UserInput {
        text: text.to_owned(),
        jpeg_screenshot: None,
    }
}

fn question_with_screenshot(text: &str) -> UserInput {
    UserInput {
        text: text.to_owned(),
        jpeg_screenshot: Some(vec![0xFF, 0xD8, 0xFF]),
    }
}

/// A server that answers every request with `deltas` and `[DONE]`.
async fn server_answering(deltas: &[&str]) -> MockServer {
    let body: String = deltas
        .iter()
        .map(|delta| format!("data: {}\n\n", json!({"choices": [{"delta": {"content": delta}}]})))
        .chain(["data: [DONE]\n\n".to_owned()])
        .collect();
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(body, "text/event-stream"))
        .mount(&server)
        .await;
    server
}

async fn server_failing_with(status: u16) -> MockServer {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(ResponseTemplate::new(status))
        .mount(&server)
        .await;
    server
}

/// A server that answers "Par" and then never finishes, like a model still thinking.
async fn stalled_server() -> ChatConfig {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base_url = format!("http://{}", listener.local_addr().unwrap());
    tokio::spawn(async move {
        loop {
            let (mut socket, _) = listener.accept().await.unwrap();
            tokio::spawn(async move {
                let event = "data: {\"choices\":[{\"delta\":{\"content\":\"Par\"}}]}\n\n";
                let response = format!(
                    "HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\ntransfer-encoding: chunked\r\n\r\n{:x}\r\n{event}\r\n",
                    event.len()
                );
                socket.write_all(response.as_bytes()).await.unwrap();
                // Reading until the client hangs up keeps the answer unfinished.
                let mut request = vec![0; 64 * 1024];
                while socket.read(&mut request).await.unwrap_or(0) > 0 {}
            });
        }
    });
    config_for(base_url)
}

async fn answer_events(
    session: &Session,
    config: &ChatConfig,
    input: UserInput,
) -> Vec<Result<AgentEvent, ProviderError>> {
    let http = reqwest::Client::new();
    session.ask(&http, config, input).collect().await
}

async fn last_request_messages(server: &MockServer) -> Vec<serde_json::Value> {
    let requests = server.received_requests().await.unwrap();
    let body: serde_json::Value = requests.last().unwrap().body_json().unwrap();
    body["messages"].as_array().unwrap().clone()
}

#[tokio::test]
async fn streams_thinking_then_the_answer_text() {
    let server = server_answering(&["Par", "is"]).await;

    let events = answer_events(&Session::default(), &config_for(server.uri()), question("Capital of France?")).await;

    assert_eq!(
        events,
        [
            Ok(AgentEvent::Thinking { forgot_earlier_turns: false }),
            Ok(AgentEvent::Delta("Par".to_owned())),
            Ok(AgentEvent::Delta("is".to_owned())),
        ]
    );
}

#[tokio::test]
async fn sends_the_system_prompt_then_the_question_with_its_screenshot() {
    let server = server_answering(&["Paris"]).await;

    answer_events(&Session::default(), &config_for(server.uri()), question_with_screenshot("What city is this?")).await;

    let messages = last_request_messages(&server).await;
    assert_eq!(messages.len(), 2);
    assert_eq!(messages[0]["role"], "system");
    assert_eq!(
        messages[1]["content"],
        json!([
            {"type": "text", "text": "What city is this?"},
            {"type": "image_url", "image_url": {"url": "data:image/jpeg;base64,/9j/"}}
        ])
    );
}

#[tokio::test]
async fn a_follow_up_carries_the_earlier_turn_as_text_without_its_screenshot() {
    let server = server_answering(&["Paris"]).await;
    let config = config_for(server.uri());
    let session = Session::default();
    answer_events(&session, &config, question_with_screenshot("What city is this?")).await;

    answer_events(&session, &config, question("How many people live there?")).await;

    let messages = last_request_messages(&server).await;
    assert_eq!(messages.len(), 4);
    assert_eq!(
        messages[1]["content"],
        json!([{"type": "text", "text": "What city is this?\n[screenshot from earlier turn]"}])
    );
    assert_eq!(messages[2]["role"], "assistant");
    assert_eq!(messages[2]["content"], json!([{"type": "text", "text": "Paris"}]));
    assert_eq!(
        messages[3]["content"],
        json!([{"type": "text", "text": "How many people live there?"}])
    );
}

#[tokio::test]
async fn a_failed_answer_is_left_out_of_the_next_question() {
    let failing = server_failing_with(500).await;
    let answering = server_answering(&["Paris"]).await;
    let session = Session::default();

    let failed = answer_events(&session, &config_for(failing.uri()), question("What city is this?")).await;
    answer_events(&session, &config_for(answering.uri()), question("Try again?")).await;

    assert!(matches!(failed.last(), Some(Err(ProviderError::Other { status: Some(500), .. }))));
    assert_eq!(last_request_messages(&answering).await.len(), 2);
}

#[tokio::test]
async fn new_chat_forgets_earlier_turns() {
    let server = server_answering(&["Paris"]).await;
    let config = config_for(server.uri());
    let session = Session::default();
    answer_events(&session, &config, question("What city is this?")).await;

    session.new_chat().await;
    answer_events(&session, &config, question("Hi")).await;

    assert_eq!(last_request_messages(&server).await.len(), 2);
}

#[tokio::test]
async fn a_new_question_stops_the_running_answer_and_keeps_its_text_so_far() {
    let stalled = stalled_server().await;
    let answering = server_answering(&["Madrid"]).await;
    let answering_config = config_for(answering.uri());
    let session = Session::default();
    let http = reqwest::Client::new();
    let mut first = std::pin::pin!(session.ask(&http, &stalled, question("What is the capital of France?")));
    assert_eq!(first.next().await, Some(Ok(AgentEvent::Thinking { forgot_earlier_turns: false })));
    assert_eq!(first.next().await, Some(Ok(AgentEvent::Delta("Par".to_owned()))));

    let second = session.ask(&http, &answering_config, question("And of Spain?"));
    let rest_of_first: Vec<_> = tokio::time::timeout(Duration::from_secs(5), first.collect())
        .await
        .expect("the running answer did not stop");
    let second_events: Vec<_> = second.collect().await;

    assert!(rest_of_first.is_empty());
    assert_eq!(second_events.last(), Some(&Ok(AgentEvent::Delta("Madrid".to_owned()))));
    let messages = last_request_messages(&answering).await;
    assert_eq!(messages.len(), 4);
    assert_eq!(messages[2]["content"], json!([{"type": "text", "text": "Par"}]));
}

#[tokio::test]
async fn new_chat_stops_the_running_answer() {
    let stalled = stalled_server().await;
    let session = Session::default();
    let http = reqwest::Client::new();
    let mut first = std::pin::pin!(session.ask(&http, &stalled, question("What is the capital of France?")));
    first.next().await;
    first.next().await;

    let (rest_of_first, ()) = tokio::time::timeout(
        Duration::from_secs(5),
        async { tokio::join!(first.collect::<Vec<_>>(), session.new_chat()) },
    )
    .await
    .expect("New chat did not stop the running answer");

    assert!(rest_of_first.is_empty());
}
```

`crates/dot-agent/src/lib.rs` becomes:

```rust
//! One conversation with the model: the turns it remembers and the one answer in flight.
//! Must not capture the screen, speak, or know about Tauri, the UI or which model serves the answer.

mod context;
mod history;
mod session;

pub use context::UserInput;
pub use session::{AgentEvent, Session};
```

- [ ] **Step 3: Run the tests to verify they fail**

Run: `cargo test -p dot-agent --test session`
Expected: FAIL to compile: `file not found for module session` or `no AgentEvent in session`.

- [ ] **Step 4: Write the implementation**

`crates/dot-agent/src/session.rs`:

```rust
//! One conversation's single answer in flight: a newer question or New chat stops the running one.
//! Must not decide which turns are remembered; history.rs does.

use std::sync::Mutex;
use std::time::Instant;

use dot_providers::{ChatConfig, ProviderError, stream_chat};
use futures_util::{Stream, StreamExt};
use tokio_util::sync::CancellationToken;

use crate::context::UserInput;
use crate::history::History;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AgentEvent {
    /// The question is on its way to the model. `forgot_earlier_turns` is true when ten idle
    /// minutes reset the conversation before it.
    Thinking { forgot_earlier_turns: bool },
    Delta(String),
}

#[derive(Default)]
pub struct Session {
    history: tokio::sync::Mutex<History>,
    running_answer: Mutex<CancellationToken>,
}

impl Session {
    /// Cancels the running answer at once. The stream ends when this answer is complete, when a
    /// newer question or `new_chat` cancels it, or after the error that stopped it.
    pub fn ask<'a>(
        &'a self,
        http: &'a reqwest::Client,
        config: &'a ChatConfig,
        input: UserInput,
    ) -> impl Stream<Item = Result<AgentEvent, ProviderError>> + 'a {
        let cancelled = self.replace_running_answer();
        async_stream::try_stream! {
            let mut history = self.history.lock().await;
            if cancelled.is_cancelled() {
                return;
            }
            let turn = history.start_turn(input, Instant::now());
            yield AgentEvent::Thinking { forgot_earlier_turns: turn.forgot_earlier_turns };
            let mut answer = std::pin::pin!(stream_chat(http, config, &turn.messages));
            while let Some(delta) = cancelled.run_until_cancelled(answer.next()).await.flatten() {
                match delta {
                    Ok(text) => {
                        history.append_to_answer(&text);
                        yield AgentEvent::Delta(text);
                    }
                    Err(error) => {
                        history.discard_running_turn();
                        Err(error)?;
                    }
                }
            }
        }
    }

    /// Stops the running answer, then forgets every turn once it has stopped.
    pub async fn new_chat(&self) {
        self.running_answer.lock().unwrap().cancel();
        self.history.lock().await.clear();
    }

    fn replace_running_answer(&self) -> CancellationToken {
        let token = CancellationToken::new();
        let running = std::mem::replace(&mut *self.running_answer.lock().unwrap(), token.clone());
        running.cancel();
        token
    }
}
```

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test -p dot-agent && cargo clippy -p dot-agent --all-targets -- -D warnings && cargo fmt --all --check`
Expected: 12 unit and 7 session tests pass. No clippy warnings. Formatting clean: run `cargo fmt --all` first if the long test lines were reflowed.

- [ ] **Step 6: Commit**

```bash
git add crates/dot-agent Cargo.lock
git commit -m "dot-agent: Session answers one question at a time, newest first"
```

---

### Task 4: a chat longer than the model's context is its own error

**Files:**
- Modify: `crates/dot-providers/src/provider_error.rs`
- Modify: `crates/dot-providers/tests/stream_chat.rs`
- Modify: `crates/dot-providers/AGENTS.md`

**Interfaces:**
- Produces: `ProviderError::ContextTooLong`, Display "the chat is longer than the model's context". Task 5 matches on it.

- [ ] **Step 1: Write the failing test**

Append to `crates/dot-providers/tests/stream_chat.rs`, after `gemini_bad_key_400_is_auth_failed`:

```rust
#[tokio::test]
async fn llama_server_context_overflow_is_context_too_long() {
    let body = r#"{"error":{"code":400,"message":"request (12012 tokens) exceeds the available context size (8192 tokens), try increasing it","type":"exceed_context_size_error","n_prompt_tokens":12012,"n_ctx":8192}}"#;
    let server = server_answering(ResponseTemplate::new(400).set_body_string(body)).await;

    assert_eq!(first_error(&server).await, ProviderError::ContextTooLong);
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p dot-providers --test stream_chat llama_server_context_overflow`
Expected: FAIL to compile: `no variant named ContextTooLong`.

- [ ] **Step 3: Write the implementation**

In `crates/dot-providers/src/provider_error.rs`, add the variant after `RateLimited`:

```rust
    #[error("the chat is longer than the model's context")]
    ContextTooLong,
```

and the arm after the Gemini arm in `error_for_status`:

```rust
        // llama-server answers a request longer than its context with 400 and this error type.
        StatusCode::BAD_REQUEST if body.contains("exceed_context_size_error") => {
            ProviderError::ContextTooLong
        }
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p dot-providers`
Expected: every test passes, including `other_400_keeps_status_and_body` (its body "context too long" does not contain `exceed_context_size_error`).

- [ ] **Step 5: Module docs**

In `crates/dot-providers/AGENTS.md`, add after the Gemini invariant:

```markdown
- llama-server answers a request longer than its context with 400 and `"type":"exceed_context_size_error"`; that maps to `ContextTooLong` so the caller can offer a new chat.
```

and replace the "Called by" line:

```markdown
Called by: `crates/dot-agent` (`Session::ask`), `app/src-tauri` (builds the `ChatConfig` and words `ProviderError` for the panel) and `crates/dot-runtime`'s ignored real-model test.
```

- [ ] **Step 6: Commit**

```bash
git add crates/dot-providers
git commit -m "dot-providers: tell a context overflow apart from other 400s"
```

---

### Task 5: ask_text asks through the Session, new_chat resets it

**Files:**
- Modify: `app/src-tauri/Cargo.toml`
- Modify: `app/src-tauri/src/commands.rs`
- Modify: `app/src-tauri/src/lib.rs`
- Modify: `app/AGENTS.md`

**Interfaces:**
- Consumes: `Session`, `AgentEvent`, `UserInput` (Task 3); `ProviderError::ContextTooLong` (Task 4).
- Produces, used by Task 6:
  - `AnswerEvent::Started` gains `forgot_earlier_turns: bool`, serialized as `forgotEarlierTurns`.
  - A new command `new_chat` that takes no arguments.

- [ ] **Step 1: Write the failing tests**

In `app/src-tauri/src/commands.rs` tests, replace `answer_events_serialize_in_the_shape_the_chat_panel_reads`'s `started` value and expected JSON:

```rust
        let started = AnswerEvent::Started {
            leaves_device: false,
            host: "127.0.0.1".to_owned(),
            screen: ScreenShare::Attached,
            forgot_earlier_turns: true,
        };
        assert_eq!(
            serde_json::to_value(&started).unwrap(),
            serde_json::json!({"event": "started", "data": {"leavesDevice": false, "host": "127.0.0.1", "screen": {"kind": "attached"}, "forgotEarlierTurns": true}})
        );
```

and append:

```rust
    #[test]
    fn a_chat_longer_than_the_context_asks_for_a_new_chat() {
        assert_eq!(
            describe_answer_error(&ProviderError::ContextTooLong),
            "This chat is too long for the model. Click New chat to start over."
        );
    }

    #[test]
    fn other_answer_errors_keep_their_own_text() {
        assert_eq!(
            describe_answer_error(&ProviderError::RateLimited),
            "the provider is rate limiting requests"
        );
    }
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p hey-dot --lib commands`
Expected: FAIL to compile: `struct variant AnswerEvent::Started has no field named forgot_earlier_turns`, `cannot find function describe_answer_error`.

- [ ] **Step 3: Write the implementation**

`app/src-tauri/Cargo.toml` `[dependencies]` gains, next to the other crates:

```toml
dot-agent = { path = "../../crates/dot-agent" }
```

In `app/src-tauri/src/commands.rs`:

1. Replace the header's second line with:

```rust
//! Must not hold state: everything lives in the managed `LocalModel`, `Session` and HTTP client.
```

2. Replace the `dot_providers` import with:

```rust
use dot_agent::{AgentEvent, Session, UserInput};
use dot_providers::ProviderError;
```

3. Add `forgot_earlier_turns: bool,` as the last field of `AnswerEvent::Started`.

4. Add after `screen_share_of`:

```rust
fn describe_answer_error(error: &ProviderError) -> String {
    match error {
        ProviderError::ContextTooLong => {
            "This chat is too long for the model. Click New chat to start over.".to_owned()
        }
        _ => error.to_string(),
    }
}
```

5. Replace `ask_text` with:

```rust
/// Resolves when the answer is complete, or when a newer question or New chat stopped it, so the
/// panel needs no end-of-answer event.
#[tauri::command]
pub async fn ask_text(
    question: String,
    local_model: State<'_, Arc<LocalModel>>,
    http: State<'_, reqwest::Client>,
    session: State<'_, Session>,
    on_event: Channel<AnswerEvent>,
) -> Result<(), String> {
    let config = local_model
        .chat_config()
        .await
        .ok_or("The local model is not ready yet.")?;
    // Read from the defaults until the settings window exists; the file is not loaded yet.
    let max_long_edge_px = Settings::default().screenshot_max_edge_px;
    let screenshot = tauri::async_runtime::spawn_blocking(move || {
        capture_display_under_cursor(max_long_edge_px)
    })
    .await
    .map_err(|error| error.to_string())?;
    let screen = screen_share_of(&screenshot);
    let input = UserInput {
        text: question,
        jpeg_screenshot: screenshot.ok(),
    };
    let mut answer = std::pin::pin!(session.ask(&http, &config, input));
    while let Some(event) = answer.next().await {
        let answer_event = match event.map_err(|error| describe_answer_error(&error))? {
            AgentEvent::Thinking {
                forgot_earlier_turns,
            } => AnswerEvent::Started {
                leaves_device: config.leaves_device(),
                host: config.base_url.host_str().unwrap_or_default().to_owned(),
                screen: screen.clone(),
                forgot_earlier_turns,
            },
            AgentEvent::Delta(text) => AnswerEvent::Delta { text },
        };
        on_event
            .send(answer_event)
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}

/// Stops the running answer; the next question starts a new chat.
#[tauri::command]
pub async fn new_chat(session: State<'_, Session>) -> Result<(), String> {
    session.new_chat().await;
    Ok(())
}
```

In `app/src-tauri/src/lib.rs`, add `app.manage(dot_agent::Session::default());` after `app.manage(local_model_http);`, and register `commands::new_chat` after `commands::ask_text` in `generate_handler!`.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `pnpm -C app build && cargo test -p hey-dot && cargo clippy -p hey-dot --all-targets -- -D warnings`
Expected: every `hey-dot` test passes, including the 2 new ones; no clippy warnings.

- [ ] **Step 5: Module docs**

In `app/AGENTS.md`, after the `ask_text captures the display...` invariant, add:

```markdown
- `ask_text` asks through the managed `dot_agent::Session`, which remembers the conversation. A newer `ask_text` or `new_chat` ends the running one early with `Ok`, so the panel treats a resolved `ask_text` as the end of that answer, finished or not. `Started` arrives only once the previous answer has stopped.
- `describe_answer_error` words `ProviderError` for the panel. A context overflow tells the user to click New chat.
```

- [ ] **Step 6: Commit**

```bash
git add app/src-tauri app/AGENTS.md Cargo.lock
git commit -m "app: ask through one dot-agent Session and add the new_chat command"
```

---

### Task 6: the panel shows the conversation, asks while streaming, starts a new chat

**Files:**
- Modify: `app/src/chat/ipc.ts`
- Modify: `app/src/chat/ChatPanel.tsx`
- Modify: `app/src/chat/ChatPanel.test.tsx`

**Interfaces:**
- Consumes: `forgotEarlierTurns` in `started` data and the `new_chat` command (Task 5).
- Produces: `newChat(): Promise<void>` in `ipc.ts`.

- [ ] **Step 1: Write the failing tests**

In `app/src/chat/ChatPanel.test.tsx`:

Replace the `started` helper with:

```ts
function started(
  screenShare: ScreenShare,
  leavesDevice = false,
  host = "127.0.0.1",
  forgotEarlierTurns = false,
): AnswerEvent {
  return { event: "started", data: { leavesDevice, host, screen: screenShare, forgotEarlierTurns } };
}

const newChatButton = () => screen.getByRole("button", { name: "New chat" });
```

Replace the test "ask stays disabled while an answer is streaming" with:

```ts
test("asking while an answer streams sends the new question and keeps the earlier one", async () => {
  const askedQuestions: string[] = [];
  await renderPanel((question, onEvent) => {
    askedQuestions.push(question);
    if (question === "What is the capital of France?") {
      onEvent.onmessage({ event: "delta", data: { text: "Par" } });
      return new Promise(() => {});
    }
    return Promise.resolve();
  });
  sendPhase({ kind: "ready" });
  typeQuestion("What is the capital of France?");
  fireEvent.click(askButton());
  expect(await screen.findByText("Par")).toBeTruthy();
  typeQuestion("And of Spain?");
  expect(askButton().disabled).toBe(false);
  fireEvent.click(askButton());
  await waitFor(() => expect(askedQuestions).toEqual(["What is the capital of France?", "And of Spain?"]));
  expect(screen.getByText("What is the capital of France?")).toBeTruthy();
  expect(screen.getByText("And of Spain?")).toBeTruthy();
});
```

Append:

```ts
test("a follow-up appears under the earlier answer", async () => {
  await renderPanel(async (question, onEvent) => {
    onEvent.onmessage(started({ kind: "attached" }));
    onEvent.onmessage({ event: "delta", data: { text: `Answer to ${question}` } });
  });
  sendPhase({ kind: "ready" });
  typeQuestion("What city is this?");
  fireEvent.click(askButton());
  expect(await screen.findByText("Answer to What city is this?")).toBeTruthy();
  typeQuestion("How many people live there?");
  fireEvent.click(askButton());
  expect(await screen.findByText("Answer to How many people live there?")).toBeTruthy();
  const turns = screen.getAllByRole("article");
  expect(turns).toHaveLength(2);
  expect(turns[0].textContent).toContain("Answer to What city is this?");
  expect(turns[1].textContent).toContain("Answer to How many people live there?");
});

test("New chat clears the conversation and tells Hey Dot to forget it", async () => {
  await renderPanel(async (_question, onEvent) => {
    onEvent.onmessage({ event: "delta", data: { text: "Paris" } });
  });
  sendPhase({ kind: "ready" });
  typeQuestion("What city is this?");
  fireEvent.click(askButton());
  expect(await screen.findByText("Paris")).toBeTruthy();
  fireEvent.click(newChatButton());
  expect(screen.queryByRole("article")).toBeNull();
  await waitFor(() => expect(invokedCommands).toContain("new_chat"));
});

test("an answer still streaming when New chat is clicked does not come back", async () => {
  let answerChannel: Channel<AnswerEvent> | undefined;
  await renderPanel((_question, onEvent) => {
    answerChannel = onEvent;
    return new Promise(() => {});
  });
  sendPhase({ kind: "ready" });
  typeQuestion("What is the capital of France?");
  fireEvent.click(askButton());
  await waitFor(() => expect(answerChannel).toBeDefined());
  fireEvent.click(newChatButton());
  act(() => answerChannel!.onmessage({ event: "delta", data: { text: "Par" } }));
  expect(screen.queryByText("Par")).toBeNull();
  expect(screen.queryByRole("article")).toBeNull();
});

test("a question Hey Dot answers after ten idle minutes hides the turns it forgot", async () => {
  await renderPanel(async (question, onEvent) => {
    const forgotEarlierTurns = question === "What is on my screen now?";
    onEvent.onmessage(started({ kind: "attached" }, false, "127.0.0.1", forgotEarlierTurns));
    onEvent.onmessage({ event: "delta", data: { text: `Answer to ${question}` } });
  });
  sendPhase({ kind: "ready" });
  typeQuestion("What city is this?");
  fireEvent.click(askButton());
  expect(await screen.findByText("Answer to What city is this?")).toBeTruthy();
  typeQuestion("What is on my screen now?");
  fireEvent.click(askButton());
  expect(await screen.findByText("Answer to What is on my screen now?")).toBeTruthy();
  expect(screen.queryByText("What city is this?")).toBeNull();
  expect(screen.getAllByRole("article")).toHaveLength(1);
});
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `pnpm -C app test`
Expected: FAIL. `tsc` rejects `forgotEarlierTurns`, which is not in the `started` data type. Once that type is added in Step 3, the new tests fail on "Unable to find role button New chat" and the disabled Ask.

- [ ] **Step 3: Write the implementation**

`app/src/chat/ipc.ts`: replace the `started` line of `AnswerEvent` with:

```ts
  | {
      event: "started";
      data: { leavesDevice: boolean; host: string; screen: ScreenShare; forgotEarlierTurns: boolean };
    }
```

replace the `askText` doc comment with:

```ts
/** Resolves when the answer is complete or a newer question or New chat stopped it; rejects with the error text when it fails. */
```

and append:

```ts
/** Stops the running answer; the next question starts a new chat. */
export function newChat(): Promise<void> {
  return invoke("new_chat");
}
```

`app/src/chat/ChatPanel.tsx`:

1. Replace the header with:

```ts
// The first chat surface: a conversation with the local model, typed one question at a time.
// Must not call invoke directly; every backend call goes through ./ipc.
```

2. Replace the react import with `import { useEffect, useRef, useState } from "react";` and add `newChat,` to the `./ipc` import list after `downloadLocalModel,`.

3. Replace the `Answer` type with:

```ts
type Turn = {
  id: number;
  question: string;
  text: string;
  badge: string | null;
  screen: ScreenShare | null;
  error: string | null;
};
```

4. Replace the `ChatPanel` component with:

```tsx
export default function ChatPanel() {
  const [status, setStatus] = useState<LocalModelStatus | null>(null);
  const [question, setQuestion] = useState("");
  const [turns, setTurns] = useState<Turn[]>([]);
  const nextTurnId = useRef(0);

  useEffect(() => {
    void watchLocalModel(setStatus);
  }, []);

  // A turn removed by New chat is not found, so its late events change nothing.
  function updateTurn(id: number, update: (turn: Turn) => Turn) {
    setTurns((current) => current.map((turn) => (turn.id === id ? update(turn) : turn)));
  }

  async function ask() {
    const id = nextTurnId.current++;
    setQuestion("");
    setTurns((current) => [...current, { id, question, text: "", badge: null, screen: null, error: null }]);
    try {
      await askText(question, (event) => {
        if (event.event === "started") {
          const { leavesDevice, host, screen, forgotEarlierTurns } = event.data;
          if (forgotEarlierTurns) {
            setTurns((current) => current.filter((turn) => turn.id >= id));
          }
          updateTurn(id, (turn) => ({ ...turn, badge: leavesDevice ? `Sent to ${host}` : "On this Mac", screen }));
        } else {
          updateTurn(id, (turn) => ({ ...turn, text: turn.text + event.data.text }));
        }
      });
    } catch (error) {
      updateTurn(id, (turn) => ({ ...turn, error: String(error) }));
    }
  }

  function startNewChat() {
    setTurns([]);
    void newChat();
  }

  const canAsk = status?.phase.kind === "ready" && question.trim() !== "";

  return (
    <section>
      {describeModel(status)}
      {turns.map((turn) => (
        <article key={turn.id}>
          <p>{turn.question}</p>
          {turn.badge && <p>{turn.badge}</p>}
          {turn.screen && describeScreenShare(turn.screen)}
          <p style={{ whiteSpace: "pre-wrap" }}>{turn.text}</p>
          {turn.error && <p role="alert">{turn.error}</p>}
        </article>
      ))}
      <label>
        Question
        <textarea value={question} onChange={(event) => setQuestion(event.target.value)} />
      </label>
      <button disabled={!canAsk} onClick={() => void ask()}>
        Ask
      </button>
      <button onClick={startNewChat}>New chat</button>
    </section>
  );
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `pnpm -C app build && pnpm -C app test`
Expected: `tsc` clean. 18 tests pass: the 14 existing ones minus the replaced one, plus 5 new.

- [ ] **Step 5: Commit**

```bash
git add app/src/chat
git commit -m "app: show the whole conversation, ask while an answer streams, New chat"
```

---

### Task 7: the real model answers a follow-up and reports an overflow

**Files:**
- Modify: `crates/dot-runtime/Cargo.toml` (dev-dependency `dot-agent`)
- Modify: `crates/dot-runtime/tests/real_model.rs`
- Modify: `crates/dot-runtime/AGENTS.md`

**Interfaces:**
- Consumes: `Session`, `AgentEvent`, `UserInput` (Task 3); `ProviderError::ContextTooLong` (Task 4).

- [ ] **Step 1: Write the tests**

`crates/dot-runtime/Cargo.toml` `[dev-dependencies]` gains:

```toml
dot-agent = { path = "../dot-agent" }
```

In `crates/dot-runtime/tests/real_model.rs`:

1. Replace the header's first line with:

```rust
//! Proves the pinned llama-server runs the real Qwen3-VL 4B GGUF and answers through dot-providers and dot-agent.
```

2. Add the imports `use dot_agent::{AgentEvent, Session, UserInput};` and change the providers import to `use dot_providers::{ChatConfig, ChatMessage, ChatRole, ProviderError, stream_chat};`.

3. Append:

```rust
async fn answer_through(session: &Session, config: &ChatConfig, question: &str) -> String {
    let asked_at = Instant::now();
    let http = reqwest::Client::new();
    let input = UserInput {
        text: question.to_owned(),
        jpeg_screenshot: None,
    };
    let answer: String = session
        .ask(&http, config, input)
        .filter_map(|event| async move {
            match event.unwrap() {
                AgentEvent::Delta(text) => Some(text),
                AgentEvent::Thinking { .. } => None,
            }
        })
        .collect()
        .await;
    eprintln!("whole answer after {:?}: {answer:?}", asked_at.elapsed());
    answer
}

#[tokio::test]
#[ignore = "needs the built llama-server and the downloaded Qwen3-VL 4B model"]
async fn the_real_4b_model_answers_a_follow_up_through_a_session() {
    let (runtime, config, _log_folder) = start_real_4b_model().await;
    let session = Session::default();
    answer_through(&session, &config, "My favorite color is teal. Reply with OK.").await;
    let follow_up = answer_through(&session, &config, "What is my favorite color? Answer in one word.").await;
    runtime.stop().await;

    assert!(follow_up.to_lowercase().contains("teal"), "{follow_up}");
}

#[tokio::test]
#[ignore = "needs the built llama-server and the downloaded Qwen3-VL 4B model"]
async fn the_real_4b_model_reports_a_chat_longer_than_its_context() {
    let (runtime, config, _log_folder) = start_real_4b_model().await;
    let question = [ChatMessage {
        role: ChatRole::User,
        text: "word ".repeat(12_000),
        jpeg_image: None,
    }];
    let http = reqwest::Client::new();
    let first_error = stream_chat(&http, &config, &question)
        .filter_map(|delta| async move { delta.err() })
        .next()
        .await;
    runtime.stop().await;

    assert_eq!(first_error, Some(ProviderError::ContextTooLong));
}
```

- [ ] **Step 2: Run the tests**

Run: `cargo test -p dot-runtime --test real_model -- --ignored --nocapture`
Expected: 4 passed. The follow-up answer contains "teal". Paste the "ready after", "first text after" and "whole answer after" lines; they are the follow-up latency benchmark for this step. These tests exercise code that already exists, so no RED step: a failure here means the model or the mapping is wrong, and gets debugged, not the test loosened.

- [ ] **Step 3: Module docs**

In `crates/dot-runtime/AGENTS.md`, replace the `tests/real_model.rs` invariant with:

```markdown
- `tests/real_model.rs` is ignored: it needs `scripts/build-llama-server.sh` run and Qwen3-VL 4B downloaded to `~/Library/Application Support/Hey Dot/models`. It proves that the pinned llama-server runs that GGUF, answers a follow-up through a `dot-agent` session, and reports a context overflow in a form `dot-providers` recognizes.
```

- [ ] **Step 4: Commit**

```bash
git add crates/dot-runtime Cargo.lock
git commit -m "dot-runtime: prove the real 4B model answers a follow-up and reports an overflow"
```

---

### Task 8: full suite, benchmark and a real click-through

**Files:** none change unless a check fails. If one fails, fix it under TDD in the owning task's files and ledger it.

- [ ] **Step 1: Full suite**

Run: `pnpm -C app build && pnpm -C app test && cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace`
Expected: all pass. Paste the tail of each.

- [ ] **Step 2: Benchmark**

The follow-up latency from Task 7, Step 2 is this step's benchmark. No benchmark exists for the session's own bookkeeping, and it is not on a hot path: it copies at most nine turns of text per question. I say so rather than invent a number.

- [ ] **Step 3: Real click-through**

Run: `pnpm -C app tauri build --debug --bundles app`, then `open "target/debug/bundle/macos/Hey Dot.app"`.

Do NOT drive the desktop: no keystrokes, no clicks, no region or full-screen screenshots. The user is using this Mac. The only capture allowed is `screencapture -l <window id>` of Hey Dot's own window. Hand the user this checklist, and ledger the results they report:
- Ask "What is the capital of France?", then "And of Spain?". Expected: both turns show, and the second answer is Madrid.
- Ask a long question ("Write 300 words about Paris."), then while it streams ask "Stop, what is 2+2?". Expected: the first answer stops, and the second answers 4.
- Click New chat, then ask "What did I ask before?". Expected: the panel is empty before the question, and the answer shows no memory of Paris.

- [ ] **Step 4: Ledger and report**

Record the suite output, the benchmark and the click-through results in the ledger. Nothing is pushed until the user has seen them.
