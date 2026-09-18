//! Writing, listing, searching and deleting saved chats and their turns.
//! Must not keep screenshots: a turn only records whether one went along.

use rusqlite::{OptionalExtension, params};

use crate::{HistoryError, HistoryStore};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ChatId(pub i64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TurnId(pub i64);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SavedChatSummary {
    pub id: ChatId,
    /// The chat's first answered question.
    pub title: String,
    /// When its latest answered question was asked.
    pub updated_at_ms: i64,
    pub turn_count: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SavedTurn {
    pub question: String,
    pub had_screenshot: bool,
    pub answer: String,
}

/// A turn counts once its answer is saved; one the app quit during stays hidden.
const ANSWERED: &str = "turns.answer <> ''";

impl HistoryStore {
    pub fn start_chat(&self) -> Result<ChatId, HistoryError> {
        self.connection
            .execute("INSERT INTO chats DEFAULT VALUES", [])?;
        Ok(ChatId(self.connection.last_insert_rowid()))
    }

    /// Saved before the answer exists, so a question is kept in the order it was asked.
    pub fn add_question(
        &self,
        chat: ChatId,
        question: &str,
        had_screenshot: bool,
        asked_at_ms: i64,
    ) -> Result<TurnId, HistoryError> {
        self.connection.execute(
            "INSERT INTO turns (chat_id, question, had_screenshot, asked_at_ms) VALUES (?1, ?2, ?3, ?4)",
            params![chat.0, question, had_screenshot, asked_at_ms],
        )?;
        Ok(TurnId(self.connection.last_insert_rowid()))
    }

    pub fn save_answer(&self, turn: TurnId, answer: &str) -> Result<(), HistoryError> {
        self.connection.execute(
            "UPDATE turns SET answer = ?2 WHERE id = ?1",
            params![turn.0, answer],
        )?;
        Ok(())
    }

    /// For a question that got no answer. Its chat goes too when nothing else is left in it.
    /// A turn already deleted with its chat is ignored, like `save_answer` ignores it.
    pub fn delete_turn(&self, turn: TurnId) -> Result<(), HistoryError> {
        let Some(chat) = self
            .connection
            .query_row(
                "DELETE FROM turns WHERE id = ?1 RETURNING chat_id",
                [turn.0],
                |row| row.get::<_, i64>(0),
            )
            .optional()?
        else {
            return Ok(());
        };
        self.connection.execute(
            "DELETE FROM chats WHERE id = ?1 AND NOT EXISTS (SELECT 1 FROM turns WHERE chat_id = ?1)",
            [chat],
        )?;
        Ok(())
    }

    /// Newest first. An empty search lists every chat; otherwise a chat is listed when any answered
    /// question or answer contains the text, ignoring ASCII case, with `%` and `_` taken literally.
    pub fn list_chats(&self, search: &str) -> Result<Vec<SavedChatSummary>, HistoryError> {
        let escaped_search = search
            .replace('\\', "\\\\")
            .replace('%', "\\%")
            .replace('_', "\\_");
        let pattern = format!("%{escaped_search}%");
        let mut statement = self.connection.prepare(&format!(
            "SELECT chats.id,
                (SELECT question FROM turns WHERE chat_id = chats.id AND {ANSWERED} ORDER BY asked_at_ms, id LIMIT 1),
                MAX(turns.asked_at_ms),
                COUNT(*)
            FROM chats JOIN turns ON turns.chat_id = chats.id AND {ANSWERED}
            GROUP BY chats.id
            HAVING SUM(turns.question LIKE ?1 ESCAPE '\\' OR turns.answer LIKE ?1 ESCAPE '\\') > 0
            ORDER BY MAX(turns.asked_at_ms) DESC, chats.id DESC"
        ))?;
        let chats = statement
            .query_map([pattern], |row| {
                Ok(SavedChatSummary {
                    id: ChatId(row.get(0)?),
                    title: row.get(1)?,
                    updated_at_ms: row.get(2)?,
                    turn_count: row.get(3)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(chats)
    }

    /// The answered turns in the order they were asked; empty for a chat that no longer exists.
    pub fn load_chat(&self, chat: ChatId) -> Result<Vec<SavedTurn>, HistoryError> {
        let mut statement = self.connection.prepare(&format!(
            "SELECT question, had_screenshot, answer FROM turns
            WHERE chat_id = ?1 AND {ANSWERED}
            ORDER BY asked_at_ms, id"
        ))?;
        let turns = statement
            .query_map([chat.0], |row| {
                Ok(SavedTurn {
                    question: row.get(0)?,
                    had_screenshot: row.get(1)?,
                    answer: row.get(2)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(turns)
    }

    pub fn delete_chat(&self, chat: ChatId) -> Result<(), HistoryError> {
        self.connection
            .execute("DELETE FROM chats WHERE id = ?1", [chat.0])?;
        Ok(())
    }

    pub fn delete_all_chats(&self) -> Result<(), HistoryError> {
        self.connection.execute("DELETE FROM chats", [])?;
        Ok(())
    }
}
