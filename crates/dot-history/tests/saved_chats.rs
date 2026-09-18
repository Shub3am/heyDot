use dot_history::{HistoryStore, SavedTurn};

const KEY: &str = "00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff";

fn open_store(folder: &tempfile::TempDir) -> HistoryStore {
    HistoryStore::open(&folder.path().join("history.db"), KEY).unwrap()
}

fn save_turn(
    store: &HistoryStore,
    chat: dot_history::ChatId,
    question: &str,
    answer: &str,
    asked_at_ms: i64,
) {
    let turn = store
        .add_question(chat, question, false, asked_at_ms)
        .unwrap();
    store.save_answer(turn, answer).unwrap();
}

#[test]
fn a_saved_chat_loads_back_in_the_order_it_was_asked() {
    let folder = tempfile::tempdir().unwrap();
    let store = open_store(&folder);
    let chat = store.start_chat().unwrap();
    let first = store.add_question(chat, "What is this?", true, 10).unwrap();
    store.save_answer(first, "A chart.").unwrap();
    save_turn(&store, chat, "And the red line?", "Revenue.", 20);

    let turns = store.load_chat(chat).unwrap();

    assert_eq!(
        turns,
        vec![
            SavedTurn {
                question: "What is this?".to_owned(),
                had_screenshot: true,
                answer: "A chart.".to_owned(),
            },
            SavedTurn {
                question: "And the red line?".to_owned(),
                had_screenshot: false,
                answer: "Revenue.".to_owned(),
            },
        ]
    );
}

#[test]
fn chats_list_newest_first_titled_by_their_first_question() {
    let folder = tempfile::tempdir().unwrap();
    let store = open_store(&folder);
    let older = store.start_chat().unwrap();
    save_turn(&store, older, "Older question", "a", 10);
    save_turn(&store, older, "Older follow up", "b", 20);
    let newer = store.start_chat().unwrap();
    save_turn(&store, newer, "Newer question", "c", 30);

    let chats = store.list_chats("").unwrap();

    assert_eq!(chats.len(), 2);
    assert_eq!(chats[0].id, newer);
    assert_eq!(chats[0].title, "Newer question");
    assert_eq!(chats[0].updated_at_ms, 30);
    assert_eq!(chats[0].turn_count, 1);
    assert_eq!(chats[1].id, older);
    assert_eq!(chats[1].title, "Older question");
    assert_eq!(chats[1].updated_at_ms, 20);
    assert_eq!(chats[1].turn_count, 2);
}

#[test]
fn turns_without_an_answer_are_left_out() {
    let folder = tempfile::tempdir().unwrap();
    let store = open_store(&folder);
    let chat = store.start_chat().unwrap();
    save_turn(&store, chat, "Answered", "Yes", 10);
    store
        .add_question(chat, "Quit mid answer", false, 20)
        .unwrap();
    let never_answered = store.start_chat().unwrap();
    store
        .add_question(never_answered, "Nothing came back", false, 30)
        .unwrap();

    let chats = store.list_chats("").unwrap();

    assert_eq!(chats.len(), 1);
    assert_eq!(chats[0].turn_count, 1);
    assert_eq!(chats[0].updated_at_ms, 10);
    assert_eq!(store.load_chat(chat).unwrap().len(), 1);
}

#[test]
fn search_matches_questions_and_answers_ignoring_case() {
    let folder = tempfile::tempdir().unwrap();
    let store = open_store(&folder);
    let by_question = store.start_chat().unwrap();
    save_turn(&store, by_question, "How do I use Figma?", "Open it.", 10);
    let by_answer = store.start_chat().unwrap();
    save_turn(&store, by_answer, "What app is this?", "That is figma.", 20);
    let unrelated = store.start_chat().unwrap();
    save_turn(&store, unrelated, "Weather?", "Sunny.", 30);

    let found: Vec<_> = store
        .list_chats("FIGMA")
        .unwrap()
        .iter()
        .map(|chat| chat.id)
        .collect();

    assert_eq!(found, vec![by_answer, by_question]);
}

#[test]
fn search_treats_wildcards_as_plain_text() {
    let folder = tempfile::tempdir().unwrap();
    let store = open_store(&folder);
    let percent = store.start_chat().unwrap();
    save_turn(&store, percent, "Is 50% off real?", "Yes.", 10);
    let underscore = store.start_chat().unwrap();
    save_turn(&store, underscore, "What is snake_case?", "A style.", 20);
    let neither = store.start_chat().unwrap();
    save_turn(&store, neither, "Plain", "Text", 30);

    assert_eq!(store.list_chats("%").unwrap().len(), 1);
    assert_eq!(store.list_chats("_").unwrap().len(), 1);
    assert_eq!(store.list_chats("\\").unwrap().len(), 0);
}

#[test]
fn deleting_a_turn_that_leaves_its_chat_empty_deletes_the_chat() {
    let folder = tempfile::tempdir().unwrap();
    let store = open_store(&folder);
    let chat = store.start_chat().unwrap();
    let failed = store.add_question(chat, "Failed", false, 10).unwrap();

    store.delete_turn(failed).unwrap();

    save_turn(&store, store.start_chat().unwrap(), "Other", "x", 20);
    assert_eq!(store.list_chats("").unwrap().len(), 1);
    assert!(store.load_chat(chat).unwrap().is_empty());
}

#[test]
fn a_turn_already_deleted_with_its_chat_is_ignored() {
    let folder = tempfile::tempdir().unwrap();
    let store = open_store(&folder);
    let chat = store.start_chat().unwrap();
    let unanswered = store.add_question(chat, "Asked", false, 10).unwrap();
    store.delete_all_chats().unwrap();

    store.save_answer(unanswered, "Late answer").unwrap();
    store.delete_turn(unanswered).unwrap();

    assert!(store.list_chats("").unwrap().is_empty());
}

#[test]
fn deleting_a_turn_keeps_the_rest_of_its_chat() {
    let folder = tempfile::tempdir().unwrap();
    let store = open_store(&folder);
    let chat = store.start_chat().unwrap();
    save_turn(&store, chat, "Kept", "Yes", 10);
    let failed = store.add_question(chat, "Failed", false, 20).unwrap();

    store.delete_turn(failed).unwrap();

    assert_eq!(store.load_chat(chat).unwrap().len(), 1);
}

#[test]
fn deleting_one_chat_keeps_the_others() {
    let folder = tempfile::tempdir().unwrap();
    let store = open_store(&folder);
    let deleted = store.start_chat().unwrap();
    save_turn(&store, deleted, "Delete me", "ok", 10);
    let kept = store.start_chat().unwrap();
    save_turn(&store, kept, "Keep me", "ok", 20);

    store.delete_chat(deleted).unwrap();

    let chats = store.list_chats("").unwrap();
    assert_eq!(chats.len(), 1);
    assert_eq!(chats[0].id, kept);
    assert!(store.load_chat(deleted).unwrap().is_empty());
}

#[test]
fn deleting_all_chats_leaves_none() {
    let folder = tempfile::tempdir().unwrap();
    let store = open_store(&folder);
    save_turn(&store, store.start_chat().unwrap(), "One", "1", 10);
    save_turn(&store, store.start_chat().unwrap(), "Two", "2", 20);

    store.delete_all_chats().unwrap();

    assert!(store.list_chats("").unwrap().is_empty());
}

#[test]
fn saving_starts_on_and_can_be_paused_across_opens() {
    let folder = tempfile::tempdir().unwrap();
    let store = open_store(&folder);
    assert!(store.is_saving().unwrap());

    store.set_saving(false).unwrap();
    drop(store);

    assert!(!open_store(&folder).is_saving().unwrap());
}
