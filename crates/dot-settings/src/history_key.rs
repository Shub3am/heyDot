//! The key that encrypts saved chats, kept in the OS credential store beside the API keys.
//! Must not log, print or persist the key anywhere else; losing it makes every saved chat unreadable.

use crate::api_keys::{ApiKeyError, KEYCHAIN_SERVICE};

const HISTORY_KEY_ACCOUNT: &str = "history-database-key";

/// 32 random bytes as 64 hex characters. The first call creates the key, every later call reads it.
pub fn read_or_create_history_key() -> Result<String, ApiKeyError> {
    let entry = keyring_core::Entry::new(KEYCHAIN_SERVICE, HISTORY_KEY_ACCOUNT)?;
    match entry.get_password() {
        Ok(history_key) => Ok(history_key),
        Err(keyring_core::Error::NoEntry) => {
            let mut key_bytes = [0u8; 32];
            getrandom::fill(&mut key_bytes).expect("macOS always has a random source");
            let history_key: String = key_bytes.iter().map(|byte| format!("{byte:02x}")).collect();
            entry.set_password(&history_key)?;
            Ok(history_key)
        }
        Err(error) => Err(error.into()),
    }
}
