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
- The sidebar has no background so the window's material shows through; the page area paints its own.
- The sidebar's first child is the 52 px drag strip under the traffic lights. It must hold no controls, and a bare `data-tauri-drag-region` drags only on direct clicks.
- Below 720 px the sidebar becomes a 64 px icon rail. `.sidebar-status` is a size container, and `localModel/ModelStatus.css` shrinks the status to its dot in it, so its Download and Try again buttons hide until the window is wider. Shell styles only its own buttons, never the ones inside the status slot.

Called by: `src/App.tsx`.
