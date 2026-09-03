//! Saved chats in an encrypted SQLite file, for the History page.
//! Must not know about the Keychain, the UI, models, or where the file lives: the caller passes the path and the key.

mod history_error;
mod history_store;
mod saved_chats;

pub use history_error::HistoryError;
pub use history_store::HistoryStore;
pub use saved_chats::{ChatId, SavedChatSummary, SavedTurn, TurnId};
