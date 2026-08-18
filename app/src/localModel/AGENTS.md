# app/src/localModel

Owns: the UI's view of the local model. One subscription to its status, and the dot plus sentence that shows it, with the Download or Try again button when one applies.

Must not know about: chats, pages or the sidebar layout. It renders wherever the caller puts it.

Entry points:
- `useLocalModelStatus()`: the latest `LocalModelStatus`, or null before the first one.
- `<ModelStatus status={...} />`.

Invariants and gotchas:
- Call `useLocalModelStatus` once, in `App.tsx`, and pass the status down. Each call opens its own `watch_local_model` channel.
- IPC calls go through `src/chat/ipc.ts`, the single TypeScript mirror of the commands.
- The sentences are the user's only explanation of a missing or broken model; the `down` sentence names the llama-server log path the Rust side writes to.

Called by: `src/App.tsx`.
