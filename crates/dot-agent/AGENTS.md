# dot-agent

Owns: one conversation with the model. It remembers the turns and builds each request from them: the system prompt, up to nine earlier turns as text, and the new question with its screenshot. It keeps one answer in flight: a newer question, Stop or New chat cancels the running answer.

Must not know about: screen capture, speech, the UI, Tauri, llama-server, or which provider is behind the `ChatConfig`. It never retries.

Entry points:
- `Session::begin_answer()`: makes a new answer the running one and returns it as a `RunningAnswer`.
- `Session::ask(RunningAnswer, &client, &config, UserInput)`: a stream of `AgentEvent`s. It ends when the answer is complete or cancelled, or after an `Err(ProviderError)` item.
- `Session::stop_answer()`: ends the running answer early and keeps the conversation.
- `Session::new_chat()`.

Invariants and gotchas:
- The spec's `Done` is the end of the stream, and its `Error(kind)` is an `Err(ProviderError)` item.
- "Last 10 turns" counts the new question, so a request carries at most nine earlier turns.
- Only the new question carries an image. An earlier question that had one ends with "[screenshot from earlier turn]" on its own line. Screenshots are never stored.
- Ten idle minutes of wall-clock time, counted from the last question and including time the Mac slept, forget every turn. A clock set back counts as no idle time. The next `Thinking` then says `forgot_earlier_turns: true`, so the UI can drop them too.
- A cancelled answer stays in history with its text so far. A failed answer is discarded. A question with no answer text is forgotten when the next question starts.
- The history lock is held for a whole answer, so `new_chat` waits until the running answer notices its cancellation. A consumer that stops polling a stream without dropping it blocks every later question.
- `begin_answer` cancels the running answer at once. Call it before slow work such as the screenshot, so Stop or New chat during that work cancel this answer. An answer cancelled before it gets the lock yields nothing at all and never reaches the model.
- A context overflow is not trimmed: the request fails with `ContextTooLong`, and the user starts a new chat.

Called by: `app/src-tauri` (the `ask_text`, `stop_answer` and `new_chat` commands) and `crates/dot-runtime`'s ignored real-model test.
