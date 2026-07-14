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
