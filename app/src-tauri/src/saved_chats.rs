//! Records the chats the user chose to save and serves the History page from the encrypted store.
//! Must not reach the Keychain or the model: the key comes from `history_commands`, the turns from `ask_text`.

use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard};
use std::time::{SystemTime, UNIX_EPOCH};

use dot_history::{ChatId, HistoryError, HistoryStore, SavedChatSummary, SavedTurn, TurnId};
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    tag = "kind"
)]
pub enum HistoryStatus {
    Off,
    On { saving: bool },
    Unavailable { reason: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SavedChatRow {
    pub id: i64,
    pub title: String,
    pub updated_at_ms: i64,
    pub turn_count: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SavedTurnRow {
    pub question: String,
    pub had_screenshot: bool,
    pub answer: String,
}

impl From<SavedChatSummary> for SavedChatRow {
    fn from(summary: SavedChatSummary) -> SavedChatRow {
        SavedChatRow {
            id: summary.id.0,
            title: summary.title,
            updated_at_ms: summary.updated_at_ms,
            turn_count: summary.turn_count,
        }
    }
}

impl From<SavedTurn> for SavedTurnRow {
    fn from(turn: SavedTurn) -> SavedTurnRow {
        SavedTurnRow {
            question: turn.question,
            had_screenshot: turn.had_screenshot,
            answer: turn.answer,
        }
    }
}

enum HistoryState {
    Off,
    On(HistoryStore),
    Unavailable(String),
}

struct Recorder {
    state: HistoryState,
    /// The saved chat the next follow-up joins.
    current_chat: Option<ChatId>,
}

impl Recorder {
    fn store(&self) -> Result<&HistoryStore, String> {
        match &self.state {
            HistoryState::On(store) => Ok(store),
            _ => Err("Saved chats are not open.".to_owned()),
        }
    }

    fn close(&mut self, reason: String) -> HistoryStatus {
        self.state = HistoryState::Unavailable(reason.clone());
        self.current_chat = None;
        HistoryStatus::Unavailable { reason }
    }

    /// Recording has nobody to show an error to, so a failed write closes the file for the page to explain.
    fn fail(&mut self, error: HistoryError) -> HistoryStatus {
        self.close(error.to_string())
    }

    fn record_question(
        &mut self,
        question: &str,
        had_screenshot: bool,
        starts_conversation: bool,
    ) -> Result<Option<TurnId>, HistoryError> {
        let HistoryState::On(store) = &self.state else {
            return Ok(None);
        };
        if !store.is_saving()? {
            return Ok(None);
        }
        let chat = match self.current_chat {
            Some(chat) if !starts_conversation => chat,
            _ => store.start_chat()?,
        };
        self.current_chat = Some(chat);
        store
            .add_question(chat, question, had_screenshot, now_ms())
            .map(Some)
    }
}

pub struct SavedChats {
    path: PathBuf,
    recorder: Mutex<Recorder>,
}

impl SavedChats {
    pub fn new(path: PathBuf) -> SavedChats {
        SavedChats {
            path,
            recorder: Mutex::new(Recorder {
                state: HistoryState::Off,
                current_chat: None,
            }),
        }
    }

    /// The file exists once saving was turned on, so its presence is the on switch across launches.
    pub fn file_exists(&self) -> bool {
        self.path.exists()
    }

    /// Creates the file on first use. Does nothing when it is already open.
    pub fn open(&self, key_hex: &str) {
        let mut recorder = self.lock();
        if matches!(recorder.state, HistoryState::On(_)) {
            return;
        }
        match self.open_store(key_hex) {
            Ok(store) => recorder.state = HistoryState::On(store),
            Err(reason) => {
                recorder.close(reason);
            }
        }
    }

    fn open_store(&self, key_hex: &str) -> Result<HistoryStore, String> {
        if let Some(folder) = self.path.parent() {
            std::fs::create_dir_all(folder).map_err(|error| error.to_string())?;
        }
        HistoryStore::open(&self.path, key_hex).map_err(|error| error.to_string())
    }

    pub fn mark_unavailable(&self, reason: String) {
        self.lock().close(reason);
    }

    pub fn status(&self) -> HistoryStatus {
        let mut recorder = self.lock();
        let saving = match &recorder.state {
            HistoryState::Off => return HistoryStatus::Off,
            HistoryState::Unavailable(reason) => {
                return HistoryStatus::Unavailable {
                    reason: reason.clone(),
                };
            }
            HistoryState::On(store) => store.is_saving(),
        };
        match saving {
            Ok(saving) => HistoryStatus::On { saving },
            Err(error) => recorder.fail(error),
        }
    }

    /// Pausing and resuming both end the current chat, so what is asked after resuming starts a new one.
    pub fn set_saving(&self, saving: bool) -> Result<(), String> {
        let mut recorder = self.lock();
        recorder
            .store()?
            .set_saving(saving)
            .map_err(|error| error.to_string())?;
        recorder.current_chat = None;
        Ok(())
    }

    /// Called at dot-agent's `Thinking`, under its conversation lock, so `starts_conversation` is in order
    /// with New chat. `None` when nothing is being saved.
    pub fn record_question(
        &self,
        question: &str,
        had_screenshot: bool,
        starts_conversation: bool,
    ) -> Option<TurnId> {
        let mut recorder = self.lock();
        match recorder.record_question(question, had_screenshot, starts_conversation) {
            Ok(turn) => turn,
            Err(error) => {
                recorder.fail(error);
                None
            }
        }
    }

    /// An empty answer deletes the question, as dot-agent forgets a turn with no answer text.
    pub fn record_answer(&self, turn: TurnId, answer: &str) {
        let mut recorder = self.lock();
        let Ok(store) = recorder.store() else {
            return;
        };
        let written = if answer.is_empty() {
            store.delete_turn(turn)
        } else {
            store.save_answer(turn, answer)
        };
        if let Err(error) = written {
            recorder.fail(error);
        }
    }

    /// For a question whose answer failed, which dot-agent does not remember either.
    pub fn forget_question(&self, turn: TurnId) {
        self.record_answer(turn, "");
    }

    pub fn list(&self, search: &str) -> Result<Vec<SavedChatRow>, String> {
        let chats = self
            .lock()
            .store()?
            .list_chats(search)
            .map_err(|error| error.to_string())?;
        Ok(chats.into_iter().map(SavedChatRow::from).collect())
    }

    /// A saved chat always has an answered turn, so no turns means it was deleted.
    pub fn load(&self, chat_id: i64) -> Result<Vec<SavedTurnRow>, String> {
        let turns = self
            .lock()
            .store()?
            .load_chat(ChatId(chat_id))
            .map_err(|error| error.to_string())?;
        if turns.is_empty() {
            return Err("This chat no longer exists.".to_owned());
        }
        Ok(turns.into_iter().map(SavedTurnRow::from).collect())
    }

    /// The next follow-up joins this chat instead of the one being saved.
    pub fn continue_chat(&self, chat_id: i64) {
        self.lock().current_chat = Some(ChatId(chat_id));
    }

    pub fn delete(&self, chat_id: i64) -> Result<(), String> {
        let mut recorder = self.lock();
        recorder
            .store()?
            .delete_chat(ChatId(chat_id))
            .map_err(|error| error.to_string())?;
        if recorder.current_chat == Some(ChatId(chat_id)) {
            recorder.current_chat = None;
        }
        Ok(())
    }

    pub fn delete_all(&self) -> Result<(), String> {
        let mut recorder = self.lock();
        recorder
            .store()?
            .delete_all_chats()
            .map_err(|error| error.to_string())?;
        recorder.current_chat = None;
        Ok(())
    }

    fn lock(&self) -> MutexGuard<'_, Recorder> {
        self.recorder.lock().unwrap()
    }
}

fn now_ms() -> i64 {
    let since_epoch = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("the clock is after 1970");
    i64::try_from(since_epoch.as_millis()).expect("milliseconds since 1970 fit in i64")
}

#[cfg(test)]
mod tests {
    use super::*;

    const KEY: &str = "00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff";
    const OTHER_KEY: &str = "ffeeddccbbaa99887766554433221100ffeeddccbbaa99887766554433221100";

    fn turned_on(folder: &tempfile::TempDir) -> SavedChats {
        let saved_chats = SavedChats::new(folder.path().join("Hey Dot/history.db"));
        saved_chats.open(KEY);
        saved_chats
    }

    fn ask(saved_chats: &SavedChats, question: &str, starts_conversation: bool) {
        let turn = saved_chats
            .record_question(question, false, starts_conversation)
            .expect("saving is on");
        saved_chats.record_answer(turn, &format!("answer to {question}"));
    }

    fn titles(saved_chats: &SavedChats) -> Vec<(String, u32)> {
        saved_chats
            .list("")
            .unwrap()
            .into_iter()
            .map(|chat| (chat.title, chat.turn_count))
            .collect()
    }

    #[test]
    fn nothing_is_saved_until_history_is_turned_on() {
        let folder = tempfile::tempdir().unwrap();
        let saved_chats = SavedChats::new(folder.path().join("history.db"));

        assert_eq!(saved_chats.status(), HistoryStatus::Off);
        assert!(saved_chats.record_question("Hi", false, true).is_none());
        assert!(!saved_chats.file_exists());
    }

    #[test]
    fn turning_on_creates_the_file_and_starts_saving() {
        let folder = tempfile::tempdir().unwrap();
        let saved_chats = turned_on(&folder);

        assert!(saved_chats.file_exists());
        assert_eq!(saved_chats.status(), HistoryStatus::On { saving: true });
    }

    #[test]
    fn follow_ups_join_the_chat_and_a_new_conversation_starts_a_new_one() {
        let folder = tempfile::tempdir().unwrap();
        let saved_chats = turned_on(&folder);

        ask(&saved_chats, "First", true);
        ask(&saved_chats, "Follow up", false);
        ask(&saved_chats, "Second chat", true);

        assert_eq!(
            titles(&saved_chats),
            vec![("Second chat".to_owned(), 1), ("First".to_owned(), 2)]
        );
    }

    #[test]
    fn a_question_without_answer_text_is_not_kept() {
        let folder = tempfile::tempdir().unwrap();
        let saved_chats = turned_on(&folder);
        let turn = saved_chats.record_question("Stopped", false, true).unwrap();

        saved_chats.record_answer(turn, "");

        assert!(titles(&saved_chats).is_empty());
    }

    #[test]
    fn a_failed_question_is_not_kept() {
        let folder = tempfile::tempdir().unwrap();
        let saved_chats = turned_on(&folder);
        ask(&saved_chats, "Kept", true);
        let failed = saved_chats.record_question("Failed", false, false).unwrap();

        saved_chats.forget_question(failed);

        assert_eq!(titles(&saved_chats), vec![("Kept".to_owned(), 1)]);
    }

    #[test]
    fn a_paused_history_saves_nothing_and_resuming_starts_a_new_chat() {
        let folder = tempfile::tempdir().unwrap();
        let saved_chats = turned_on(&folder);
        ask(&saved_chats, "Before pause", true);

        saved_chats.set_saving(false).unwrap();
        assert!(
            saved_chats
                .record_question("Private", false, false)
                .is_none()
        );
        saved_chats.set_saving(true).unwrap();
        ask(&saved_chats, "After pause", false);

        assert_eq!(saved_chats.status(), HistoryStatus::On { saving: true });
        assert_eq!(
            titles(&saved_chats),
            vec![
                ("After pause".to_owned(), 1),
                ("Before pause".to_owned(), 1)
            ]
        );
    }

    #[test]
    fn a_continued_chat_takes_the_next_follow_up() {
        let folder = tempfile::tempdir().unwrap();
        let saved_chats = turned_on(&folder);
        ask(&saved_chats, "Old chat", true);
        let old_chat = saved_chats.list("").unwrap()[0].id;
        ask(&saved_chats, "Other chat", true);

        let old_turns = saved_chats.load(old_chat).unwrap();
        saved_chats.continue_chat(old_chat);
        ask(&saved_chats, "Back to it", false);

        assert_eq!(old_turns.len(), 1);
        assert_eq!(old_turns[0].question, "Old chat");
        assert_eq!(
            titles(&saved_chats),
            vec![("Old chat".to_owned(), 2), ("Other chat".to_owned(), 1)]
        );
    }

    #[test]
    fn a_deleted_chat_does_not_load() {
        let folder = tempfile::tempdir().unwrap();
        let saved_chats = turned_on(&folder);
        ask(&saved_chats, "Deleted", true);
        let deleted_chat = saved_chats.list("").unwrap()[0].id;
        saved_chats.delete(deleted_chat).unwrap();

        assert_eq!(
            saved_chats.load(deleted_chat),
            Err("This chat no longer exists.".to_owned())
        );
    }

    #[test]
    fn deleting_the_chat_being_saved_sends_the_next_follow_up_to_a_new_chat() {
        let folder = tempfile::tempdir().unwrap();
        let saved_chats = turned_on(&folder);
        ask(&saved_chats, "Deleted", true);

        saved_chats
            .delete(saved_chats.list("").unwrap()[0].id)
            .unwrap();
        ask(&saved_chats, "Follow up", false);

        assert_eq!(titles(&saved_chats), vec![("Follow up".to_owned(), 1)]);
    }

    #[test]
    fn deleting_every_chat_sends_the_next_follow_up_to_a_new_chat() {
        let folder = tempfile::tempdir().unwrap();
        let saved_chats = turned_on(&folder);
        ask(&saved_chats, "One", true);
        ask(&saved_chats, "Two", true);

        saved_chats.delete_all().unwrap();
        ask(&saved_chats, "Follow up", false);

        assert_eq!(titles(&saved_chats), vec![("Follow up".to_owned(), 1)]);
    }

    #[test]
    fn a_key_that_does_not_open_the_file_leaves_history_unavailable() {
        let folder = tempfile::tempdir().unwrap();
        drop(turned_on(&folder));
        let reopened = SavedChats::new(folder.path().join("Hey Dot/history.db"));

        reopened.open(OTHER_KEY);

        assert_eq!(
            reopened.status(),
            HistoryStatus::Unavailable {
                reason: "the saved chats could not be unlocked with the key in the Keychain"
                    .to_owned()
            }
        );
        assert!(reopened.record_question("Hi", false, true).is_none());
    }

    #[test]
    fn statuses_serialize_in_the_shape_the_history_page_reads() {
        assert_eq!(
            serde_json::to_value(HistoryStatus::Off).unwrap(),
            serde_json::json!({"kind": "off"})
        );
        assert_eq!(
            serde_json::to_value(HistoryStatus::On { saving: false }).unwrap(),
            serde_json::json!({"kind": "on", "saving": false})
        );
        assert_eq!(
            serde_json::to_value(HistoryStatus::Unavailable {
                reason: "locked".to_owned()
            })
            .unwrap(),
            serde_json::json!({"kind": "unavailable", "reason": "locked"})
        );
    }

    #[test]
    fn saved_chats_serialize_in_the_shape_the_history_page_reads() {
        let chat = SavedChatRow {
            id: 7,
            title: "What is this?".to_owned(),
            updated_at_ms: 1_700_000_000_000,
            turn_count: 2,
        };
        let turn = SavedTurnRow {
            question: "What is this?".to_owned(),
            had_screenshot: true,
            answer: "A chart.".to_owned(),
        };

        assert_eq!(
            serde_json::to_value(chat).unwrap(),
            serde_json::json!({"id": 7, "title": "What is this?", "updatedAtMs": 1_700_000_000_000_i64, "turnCount": 2})
        );
        assert_eq!(
            serde_json::to_value(turn).unwrap(),
            serde_json::json!({"question": "What is this?", "hadScreenshot": true, "answer": "A chart."})
        );
    }
}
