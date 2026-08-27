# app/src/history

Owns: the History page. Today it only says what saved chats will be and how long a chat lasts now.

Must not know about: the running conversation or the sidebar. It offers no button until saved chats work.

Entry points: `<HistoryPage />`.

Invariants and gotchas:
- The "today" sentence must match the backend: a chat ends on New chat, ten idle minutes, or quitting. Change it when `dot-agent`'s idle timeout changes.

Called by: `src/App.tsx`.
