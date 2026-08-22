// The labelled sidebar: New chat, one button per page, and a slot at the bottom for the model status.
// Must not know how a chat works or what a page shows; App.tsx decides what each button does.

import type { ReactNode } from "react";
import { NewChatIcon, PAGE_ICONS } from "./icons";
import { PAGES, type Page } from "./pages";
import "./shell.css";

type SidebarProps = {
  currentPage: Page;
  onOpenPage: (page: Page) => void;
  onNewChat: () => void;
  children: ReactNode;
};

export default function Sidebar({ currentPage, onOpenPage, onNewChat, children }: SidebarProps) {
  return (
    <nav className="sidebar" aria-label="Pages">
      {/* The macOS traffic lights sit over this strip, so it holds no controls; dragging it moves the window. */}
      <div className="sidebar-drag-strip" data-tauri-drag-region />
      <button className="sidebar-new-chat" aria-label="New chat" title="New chat" onClick={onNewChat}>
        <NewChatIcon />
        <span className="sidebar-label">New chat</span>
      </button>
      <ul className="sidebar-pages">
        {PAGES.map(({ page, label }) => {
          const PageIcon = PAGE_ICONS[page];
          return (
            <li key={page}>
              <button
                aria-label={label}
                title={label}
                aria-current={page === currentPage ? "page" : undefined}
                onClick={() => onOpenPage(page)}
              >
                <PageIcon />
                <span className="sidebar-label">{label}</span>
              </button>
            </li>
          );
        })}
      </ul>
      <div className="sidebar-status">{children}</div>
    </nav>
  );
}
