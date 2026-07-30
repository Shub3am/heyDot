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
