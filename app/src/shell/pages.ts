// The pages the sidebar switches between, in sidebar order.
// Must not know what any page shows.

export type Page = "chat" | "history" | "documents" | "settings";

export const PAGES: { page: Page; label: string }[] = [
  { page: "chat", label: "Chat" },
  { page: "history", label: "History" },
  { page: "documents", label: "Documents" },
  { page: "settings", label: "Settings" },
];
