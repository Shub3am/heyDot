//! The IPC commands the chat panel calls.
//! Must not hold state: everything lives in the managed `LocalModel`, `Session` and HTTP client.

use std::sync::Arc;

use dot_agent::{AgentEvent, Session, UserInput};
use dot_providers::ProviderError;
use dot_screen::{ScreenError, capture_display_under_cursor, request_screen_access};
use dot_settings::Settings;
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
    Started {
        leaves_device: bool,
        host: String,
        screen: ScreenShare,
        forgot_earlier_turns: bool,
    },
    Delta {
        text: String,
    },
}

/// Whether the question's screenshot went along, and if not, why.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    tag = "kind"
)]
pub enum ScreenShare {
    Attached,
    PermissionNeeded,
    Failed { reason: String },
}

fn screen_share_of(capture: &Result<Vec<u8>, ScreenError>) -> ScreenShare {
    match capture {
        Ok(_) => ScreenShare::Attached,
        Err(ScreenError::PermissionNotGranted) => ScreenShare::PermissionNeeded,
        Err(error) => ScreenShare::Failed {
            reason: error.to_string(),
        },
    }
}

fn describe_answer_error(error: &ProviderError) -> String {
    match error {
        ProviderError::ContextTooLong => {
            "This chat is too long for the model. Click New chat to start over.".to_owned()
        }
        _ => error.to_string(),
    }
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

/// Resolves when the answer is complete, or when a newer question, Stop or New chat stopped it,
/// so the panel needs no end-of-answer event.
#[tauri::command]
pub async fn ask_text(
    question: String,
    local_model: State<'_, Arc<LocalModel>>,
    http: State<'_, reqwest::Client>,
    session: State<'_, Session>,
    on_event: Channel<AnswerEvent>,
) -> Result<(), String> {
    let running_answer = session.begin_answer();
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
    let mut answer = std::pin::pin!(session.ask(running_answer, &http, &config, input));
    while let Some(event) = answer.next().await {
        let answer_event = match event.map_err(|error| describe_answer_error(&error))? {
            AgentEvent::Thinking {
                forgot_earlier_turns,
                ..
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

/// Stops the running answer; the next question still follows up on it.
#[tauri::command]
pub fn stop_answer(session: State<'_, Session>) {
    session.stop_answer();
}

/// Stops the running answer; the next question starts a new chat.
#[tauri::command]
pub async fn new_chat(session: State<'_, Session>) -> Result<(), String> {
    session.new_chat().await;
    Ok(())
}

/// The permission card's button. macOS only lists an app under Screen Recording after it asked
/// once, so this asks first, then opens the pane where the user turns Hey Dot on.
#[tauri::command]
pub fn open_screen_recording_settings() -> Result<(), String> {
    request_screen_access();
    std::process::Command::new("open")
        .arg("x-apple.systempreferences:com.apple.preference.security?Privacy_ScreenCapture")
        .spawn()
        .map(drop)
        .map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn answer_events_serialize_in_the_shape_the_chat_panel_reads() {
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
        assert_eq!(
            serde_json::to_value(AnswerEvent::Delta {
                text: "Par".to_owned()
            })
            .unwrap(),
            serde_json::json!({"event": "delta", "data": {"text": "Par"}})
        );
    }

    #[test]
    fn screen_shares_serialize_in_the_shape_the_chat_panel_reads() {
        assert_eq!(
            serde_json::to_value(ScreenShare::PermissionNeeded).unwrap(),
            serde_json::json!({"kind": "permissionNeeded"})
        );
        assert_eq!(
            serde_json::to_value(ScreenShare::Failed {
                reason: "no display is under the mouse cursor".to_owned()
            })
            .unwrap(),
            serde_json::json!({"kind": "failed", "reason": "no display is under the mouse cursor"})
        );
    }

    #[test]
    fn a_missing_permission_asks_for_it() {
        assert_eq!(
            screen_share_of(&Err(ScreenError::PermissionNotGranted)),
            ScreenShare::PermissionNeeded
        );
    }

    #[test]
    fn any_other_capture_failure_shows_its_reason() {
        assert_eq!(
            screen_share_of(&Err(ScreenError::NoDisplayUnderCursor)),
            ScreenShare::Failed {
                reason: "no display is under the mouse cursor".to_owned()
            }
        );
    }

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
}
