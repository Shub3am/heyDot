use dot_settings::read_or_create_history_key;

#[test]
fn the_history_key_is_created_once_then_read_back() {
    keyring_core::set_default_store(keyring_core::mock::Store::new().unwrap());

    let created = read_or_create_history_key().unwrap();
    let read_back = read_or_create_history_key().unwrap();

    assert_eq!(created.len(), 64);
    assert!(
        created
            .chars()
            .all(|character| character.is_ascii_hexdigit())
    );
    assert_eq!(read_back, created);
}
