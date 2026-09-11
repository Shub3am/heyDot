# dot-history

Owns: the saved-chats file: an SQLCipher-encrypted SQLite database of chats and their turns (question, answer, whether a screenshot went along), searching and deleting them, and the switch that pauses saving.

Must not know about: the Keychain, the UI, dot-agent, models, or where the file lives. The caller passes the path and the key; the app uses `~/Library/Application Support/Hey Dot/history.db` and the key from `dot_settings::read_or_create_history_key`.

Entry points: `HistoryStore::open(path, key_hex)`, then `start_chat`, `add_question`, `save_answer`, `delete_turn` to record; `list_chats(search)`, `load_chat`, `delete_chat`, `delete_all_chats` for the History page; `is_saving` and `set_saving`.

Invariants and gotchas:
- Screenshots are never stored. A turn keeps only `had_screenshot`.
- The key is applied raw (`x'<hex>'`), so SQLCipher skips PBKDF2. It is validated as 64 hex characters before it reaches SQL, and before the file is created.
- A wrong key shows up on the first read as SQLITE_NOTADB and is returned as `WrongKey`. There is no recovery: a lost key means the chats are gone.
- A question is written before its answer exists. A turn with an empty answer (the app quit mid-answer) is hidden from lists and loads, and a chat with no answered turn is not listed.
- `save_answer` and `delete_turn` on a turn already deleted with its chat do nothing and succeed: the page can delete a chat while its answer is still running.
- A chat's title and date are derived from its answered turns: the first question, and when the latest was asked. Nothing stores them separately.
- Ids use AUTOINCREMENT so a deleted chat's id is never reused: the app and the page hold ids across deletes.
- `secure_delete` is on, so deleted text is overwritten in the file.
- Search is LIKE over questions and answers: case-insensitive for ASCII only, `%` and `_` literal. No full-text index; fine for thousands of chats.
- Schema version lives in `PRAGMA user_version`. A newer version is `NewerFormat` and is never touched.
- Every call blocks and `HistoryStore` is not `Sync`: the caller keeps it behind a mutex.

Called by: `app/src-tauri` (recording answers and the History commands).
