//! Why the saved chats could not be opened or changed.
//! Must not carry the key or any saved text in its messages.

#[derive(Debug, thiserror::Error)]
pub enum HistoryError {
    #[error("the saved chats key must be 64 hex characters")]
    InvalidKey,
    #[error("the saved chats could not be unlocked with the key in the Keychain")]
    WrongKey,
    #[error("the saved chats were written by a newer Hey Dot")]
    NewerFormat,
    #[error("saved chats database error: {0}")]
    Database(#[from] rusqlite::Error),
}
