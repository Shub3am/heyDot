use std::sync::Once;

use dot_settings::{read_api_key, save_api_key};

// The default credential store is process-global and tests run in parallel threads, so it
// is set once and each test uses its own provider id.
fn use_mock_keychain() {
    static MOCK_KEYCHAIN: Once = Once::new();
    MOCK_KEYCHAIN.call_once(|| {
        keyring_core::set_default_store(keyring_core::mock::Store::new().unwrap());
    });
}

#[test]
fn saved_key_reads_back() {
    use_mock_keychain();

    save_api_key("provider-saved", "sk-first").unwrap();

    assert_eq!(
        read_api_key("provider-saved").unwrap(),
        Some("sk-first".to_string())
    );
}

#[test]
fn saving_again_replaces_the_key() {
    use_mock_keychain();

    save_api_key("provider-replaced", "sk-old").unwrap();
    save_api_key("provider-replaced", "sk-new").unwrap();

    assert_eq!(
        read_api_key("provider-replaced").unwrap(),
        Some("sk-new".to_string())
    );
}

#[test]
fn provider_without_a_key_reads_none() {
    use_mock_keychain();

    assert_eq!(read_api_key("provider-never-saved").unwrap(), None);
}
