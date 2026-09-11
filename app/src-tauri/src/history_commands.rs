//! The IPC commands the History page calls.
//! Must not record turns or hold state: `ask_text` records through the managed `SavedChats`.

use dot_agent::{PastTurn, Session};
use tauri::State;

use crate::saved_chats::{HistoryStatus, SavedChatRow, SavedChats, SavedTurnRow};

/// Reads the key from the Keychain, creating it the first time, and opens the file with it.
pub fn unlock_saved_chats(saved_chats: &SavedChats) {
    match dot_settings::read_or_create_history_key() {
        Ok(history_key) => saved_chats.open(&history_key),
        Err(error) => saved_chats.mark_unavailable(error.to_string()),
    }
}

#[tauri::command]
pub async fn history_status(saved_chats: State<'_, SavedChats>) -> Result<HistoryStatus, String> {
    Ok(saved_chats.status())
}

#[tauri::command]
pub async fn turn_on_history(saved_chats: State<'_, SavedChats>) -> Result<HistoryStatus, String> {
    unlock_saved_chats(&saved_chats);
    Ok(saved_chats.status())
}

#[tauri::command]
pub async fn set_history_saving(
    saving: bool,
    saved_chats: State<'_, SavedChats>,
) -> Result<HistoryStatus, String> {
    saved_chats.set_saving(saving)?;
    Ok(saved_chats.status())
}

#[tauri::command]
pub async fn list_saved_chats(
    search: String,
    saved_chats: State<'_, SavedChats>,
) -> Result<Vec<SavedChatRow>, String> {
    saved_chats.list(&search)
}

/// The next question follows up on the chat's last turns and is saved into it. The session resumes
/// before the chat becomes current, so a question racing the open is saved where it was asked.
#[tauri::command]
pub async fn open_saved_chat(
    chat_id: i64,
    saved_chats: State<'_, SavedChats>,
    session: State<'_, Session>,
) -> Result<Vec<SavedTurnRow>, String> {
    let turns = saved_chats.load(chat_id)?;
    let past_turns = turns
        .iter()
        .map(|turn| PastTurn {
            question: turn.question.clone(),
            had_screenshot: turn.had_screenshot,
            answer: turn.answer.clone(),
        })
        .collect();
    session.resume(past_turns).await;
    saved_chats.continue_chat(chat_id);
    Ok(turns)
}

#[tauri::command]
pub async fn delete_saved_chat(
    chat_id: i64,
    saved_chats: State<'_, SavedChats>,
) -> Result<(), String> {
    saved_chats.delete(chat_id)
}

#[tauri::command]
pub async fn delete_all_saved_chats(saved_chats: State<'_, SavedChats>) -> Result<(), String> {
    saved_chats.delete_all()
}
