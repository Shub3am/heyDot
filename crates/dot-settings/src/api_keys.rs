//! Cloud provider API keys, kept in the OS credential store so they never reach the
//! settings file or logs.
//! Must not log, print or persist a key anywhere; `read_api_key`'s return value is the
//! only way a key leaves this file.

const KEYCHAIN_SERVICE: &str = "com.shub3am.heydot";

#[derive(Debug, thiserror::Error)]
#[error("keychain error: {0}")]
pub struct ApiKeyError(#[from] keyring_core::Error);

/// Called once at app start, before any key is read or saved.
pub fn use_macos_keychain() -> Result<(), ApiKeyError> {
    keyring_core::set_default_store(apple_native_keyring_store::keychain::Store::new()?);
    Ok(())
}

pub fn save_api_key(provider_id: &str, api_key: &str) -> Result<(), ApiKeyError> {
    keyring_core::Entry::new(KEYCHAIN_SERVICE, provider_id)?.set_password(api_key)?;
    Ok(())
}

pub fn read_api_key(provider_id: &str) -> Result<Option<String>, ApiKeyError> {
    match keyring_core::Entry::new(KEYCHAIN_SERVICE, provider_id)?.get_password() {
        Ok(api_key) => Ok(Some(api_key)),
        Err(keyring_core::Error::NoEntry) => Ok(None),
        Err(error) => Err(error.into()),
    }
}
