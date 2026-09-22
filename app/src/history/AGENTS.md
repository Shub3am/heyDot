# app/src/history

Owns: the History page. It turns saving on, pauses and resumes it, searches the saved chats, and opens or deletes them.

Must not know about: the running conversation, the sidebar, or how a chat's turns are drawn. Opening a chat hands its turns to `onOpenChat`; `App.tsx` shows them on the Chat page. Rust is reached only through `../chat/ipc.ts`.

Entry points: `<HistoryPage onOpenChat={...} />`.

Invariants and gotchas:
- The "A chat lasts until..." sentence shown while saving is off must match the backend: a chat ends on New chat, ten idle minutes, or quitting. Change it when `dot-agent`'s idle timeout changes.
- Every open, delete or delete-all is followed by a fresh list, success or failure, so a chat deleted elsewhere drops out of the page. `open_saved_chat` rejects a chat that no longer exists.
- Only the reply to the latest search may set the list; an older reply that lands late is ignored.
- Delete all asks for a second click; a single chat's Delete does not.
- Unavailable saved chats show the reason and no buttons: nothing on the page can fix a missing key.

Called by: `src/App.tsx`.
