// The sidebar's icons, drawn inline so nothing is fetched at launch.
// Must not carry meaning on their own; every button that shows one also has its label.

import type { ReactElement } from "react";
import type { Page } from "./pages";

function Icon({ children }: { children: ReactElement | ReactElement[] }) {
  return (
    <svg
      aria-hidden="true"
      width="16"
      height="16"
      viewBox="0 0 16 16"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.4"
      strokeLinecap="round"
      strokeLinejoin="round"
    >
      {children}
    </svg>
  );
}

export function NewChatIcon() {
  return (
    <Icon>
      <path d="M8 3v10M3 8h10" />
    </Icon>
  );
}

export const PAGE_ICONS: Record<Page, () => ReactElement> = {
  chat: () => (
    <Icon>
      <path d="M2.5 4.5a2 2 0 0 1 2-2h7a2 2 0 0 1 2 2v5a2 2 0 0 1-2 2H7l-3 2.5v-2.5h0.5a2 2 0 0 1-2-2z" />
    </Icon>
  ),
  history: () => (
    <Icon>
      <circle cx="8" cy="8" r="5.5" />
      <path d="M8 5v3l2 1.5" />
    </Icon>
  ),
  documents: () => (
    <Icon>
      <path d="M4 1.5h5l3 3v10H4z" />
      <path d="M9 1.5v3h3M6 8.5h4M6 11h4" />
    </Icon>
  ),
  settings: () => (
    <Icon>
      <circle cx="8" cy="8" r="2" />
      <path d="M8 1.5v2M8 12.5v2M1.5 8h2M12.5 8h2M3.4 3.4l1.4 1.4M11.2 11.2l1.4 1.4M3.4 12.6l1.4-1.4M11.2 4.8l1.4-1.4" />
    </Icon>
  ),
};
