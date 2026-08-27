# app/src/settings

Owns: the Settings page. Today it says which settings are coming and offers the one that works: opening macOS Screen Recording settings.

Must not know about: chats or the sidebar. It offers a button only for a setting that works.

Entry points: `<SettingsPage />`.

Invariants and gotchas:
- IPC calls go through `src/chat/ipc.ts`, the single TypeScript mirror of the commands.

Called by: `src/App.tsx`.
