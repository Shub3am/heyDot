# app/src/documents

Owns: the Documents page. Today it only says what answering from documents will be and what answers use now.

Must not know about: chats or the sidebar. It offers no button until document search works.

Entry points: `<DocumentsPage />`.

Invariants and gotchas:
- The "today" sentence must stay true: answers use the screen and the chat only, no files.

Called by: `src/App.tsx`.
