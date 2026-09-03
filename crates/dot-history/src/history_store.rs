//! Opens the encrypted saved-chats file, creates its tables, and holds the switch that pauses saving.
//! Must not decide when a turn is saved: the caller asks for every write.

use std::path::Path;

use rusqlite::{Connection, ErrorCode};

use crate::HistoryError;

const SCHEMA_VERSION: i64 = 1;

// AUTOINCREMENT so a deleted chat's id is never handed to a new one: the app and the page hold ids across deletes.
const CREATE_SCHEMA: &str = "
    BEGIN;
    CREATE TABLE chats (id INTEGER PRIMARY KEY AUTOINCREMENT);
    CREATE TABLE turns (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        chat_id INTEGER NOT NULL REFERENCES chats(id) ON DELETE CASCADE,
        question TEXT NOT NULL,
        answer TEXT NOT NULL DEFAULT '',
        had_screenshot INTEGER NOT NULL,
        asked_at_ms INTEGER NOT NULL
    );
    CREATE INDEX turns_by_chat ON turns(chat_id);
    CREATE TABLE state (saving INTEGER NOT NULL);
    INSERT INTO state (saving) VALUES (1);
    PRAGMA user_version = 1;
    COMMIT;
";

/// One open saved-chats file. Blocking; not `Sync`, so the caller keeps it behind a mutex.
pub struct HistoryStore {
    pub(crate) connection: Connection,
}

impl HistoryStore {
    /// Creates the file on first use. `key_hex` is 32 bytes as 64 hex characters.
    pub fn open(path: &Path, key_hex: &str) -> Result<HistoryStore, HistoryError> {
        if key_hex.len() != 64 || !key_hex.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err(HistoryError::InvalidKey);
        }
        let connection = Connection::open(path)?;
        // SQLCipher takes a raw 256-bit key in this form and skips its PBKDF2 pass, which a random key does not need.
        connection.execute_batch(&format!("PRAGMA key = \"x'{key_hex}'\";"))?;
        // SQLCipher only notices a wrong key on the first read, as "file is not a database".
        let schema_version: i64 = connection
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .map_err(|error| match error.sqlite_error_code() {
                Some(ErrorCode::NotADatabase) => HistoryError::WrongKey,
                _ => HistoryError::Database(error),
            })?;
        // Deleted chats are overwritten on disk instead of lingering in free pages.
        connection.execute_batch("PRAGMA foreign_keys = ON; PRAGMA secure_delete = ON;")?;
        match schema_version {
            0 => connection.execute_batch(CREATE_SCHEMA)?,
            SCHEMA_VERSION => {}
            _ => return Err(HistoryError::NewerFormat),
        }
        Ok(HistoryStore { connection })
    }

    pub fn is_saving(&self) -> Result<bool, HistoryError> {
        Ok(self
            .connection
            .query_row("SELECT saving FROM state", [], |row| row.get(0))?)
    }

    pub fn set_saving(&self, saving: bool) -> Result<(), HistoryError> {
        self.connection
            .execute("UPDATE state SET saving = ?1", [saving])?;
        Ok(())
    }
}
