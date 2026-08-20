# app/src/shell

Owns: the frame around the pages. The sidebar (New chat, one button per page, the model status slot), the list of pages, and the page header and card styles every page uses.

Must not know about: how chats, history, documents or settings work, or any IPC command. It renders what `App.tsx` hands it.

Entry points:
- `<Sidebar currentPage onOpenPage onNewChat>{status}</Sidebar>`.
- `PAGES` and the `Page` type in `pages.ts`, the one place the page order and labels live.
- `shell.css`: `.page`, `.page-header` and `.page-card` for any page.

Invariants and gotchas:
- The Chat page stays mounted on other pages so a streaming answer survives the switch. `App.tsx` hides it with `hidden` on `.page-slot`, which has no display rule; a display rule there would override `hidden` and show both pages.
- New chat remounts the Chat page through its `key` and calls `newChat`, so a late answer from the old chat has no panel to land in.
- Every sidebar button carries `aria-label` and `title`, because its text is hidden when the window is narrow enough to show only icons.
- `aria-current="page"` marks the open page; the CSS keys the highlight off it.

Called by: `src/App.tsx`.
