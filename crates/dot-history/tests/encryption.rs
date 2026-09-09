use dot_history::{HistoryError, HistoryStore};

const KEY: &str = "00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff";
const OTHER_KEY: &str = "ffeeddccbbaa99887766554433221100ffeeddccbbaa99887766554433221100";

#[test]
fn the_file_on_disk_is_not_readable_as_plain_sqlite() {
    let folder = tempfile::tempdir().unwrap();
    let path = folder.path().join("history.db");
    let store = HistoryStore::open(&path, KEY).unwrap();
    let chat = store.start_chat().unwrap();
    let turn = store
        .add_question(chat, "What is my bank balance?", false, 1)
        .unwrap();
    store.save_answer(turn, "Forty two").unwrap();
    drop(store);

    let file_bytes = std::fs::read(&path).unwrap();
    assert!(!file_bytes.starts_with(b"SQLite format 3\0"));
    let file_text = String::from_utf8_lossy(&file_bytes);
    assert!(!file_text.contains("bank balance"));
}

#[test]
fn a_different_key_cannot_open_the_file() {
    let folder = tempfile::tempdir().unwrap();
    let path = folder.path().join("history.db");
    drop(HistoryStore::open(&path, KEY).unwrap());

    let opened = HistoryStore::open(&path, OTHER_KEY);

    assert!(matches!(opened, Err(HistoryError::WrongKey)));
}

#[test]
fn the_same_key_opens_the_file_again() {
    let folder = tempfile::tempdir().unwrap();
    let path = folder.path().join("history.db");
    let store = HistoryStore::open(&path, KEY).unwrap();
    let chat = store.start_chat().unwrap();
    let turn = store.add_question(chat, "Hi", false, 1).unwrap();
    store.save_answer(turn, "Hello").unwrap();
    drop(store);

    let reopened = HistoryStore::open(&path, KEY).unwrap();

    assert_eq!(reopened.list_chats("").unwrap().len(), 1);
}

#[test]
fn a_key_that_is_not_64_hex_characters_is_refused() {
    let folder = tempfile::tempdir().unwrap();
    let path = folder.path().join("history.db");

    let opened = HistoryStore::open(&path, "x'); DROP TABLE chats; --");

    assert!(matches!(opened, Err(HistoryError::InvalidKey)));
    assert!(!path.exists());
}
