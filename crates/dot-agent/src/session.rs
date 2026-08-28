//! One conversation's single answer in flight: a newer question, Stop or New chat stops the running one.
//! Must not decide which turns are remembered; history.rs does.

use std::sync::Mutex;
use std::time::SystemTime;

use dot_providers::{ChatConfig, ProviderError, stream_chat};
use futures_util::{Stream, StreamExt};
use tokio_util::sync::CancellationToken;

use crate::context::UserInput;
use crate::history::History;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AgentEvent {
    /// The question is on its way to the model. `forgot_earlier_turns` is true when ten idle
    /// minutes reset the conversation before it.
    Thinking {
        forgot_earlier_turns: bool,
    },
    Delta(String),
}

/// An answer that is now the running one, so Stop, New chat or a newer question can cancel it
/// before `ask` sends it.
pub struct RunningAnswer(CancellationToken);

#[derive(Default)]
pub struct Session {
    history: tokio::sync::Mutex<History>,
    running_answer: Mutex<CancellationToken>,
}

impl Session {
    /// Cancels the running answer and puts a new one in its place. Take it before slow work such
    /// as the screenshot, so a Stop or New chat during that work cancels this answer.
    pub fn begin_answer(&self) -> RunningAnswer {
        let token = CancellationToken::new();
        let running = std::mem::replace(&mut *self.running_answer.lock().unwrap(), token.clone());
        running.cancel();
        RunningAnswer(token)
    }

    /// Sends the question for `answer`. The stream ends when the answer is complete, when a newer
    /// question, `stop_answer` or `new_chat` cancels it, or after the error that stopped it. It is
    /// empty when the answer was cancelled before it was sent.
    pub fn ask<'a>(
        &'a self,
        answer: RunningAnswer,
        http: &'a reqwest::Client,
        config: &'a ChatConfig,
        input: UserInput,
    ) -> impl Stream<Item = Result<AgentEvent, ProviderError>> + 'a {
        let RunningAnswer(cancelled) = answer;
        async_stream::try_stream! {
            let mut history = self.history.lock().await;
            if cancelled.is_cancelled() {
                return;
            }
            let turn = history.start_turn(input, SystemTime::now());
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

    /// Stops the running answer. Its text so far stays in the conversation.
    pub fn stop_answer(&self) {
        self.running_answer.lock().unwrap().cancel();
    }

    /// Stops the running answer, then forgets every turn once it has stopped.
    pub async fn new_chat(&self) {
        self.running_answer.lock().unwrap().cancel();
        self.history.lock().await.clear();
    }
}
