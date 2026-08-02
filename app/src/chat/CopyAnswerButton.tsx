// Copies one answer's markdown to the clipboard and says so for a moment.
// Must not format or change the answer; it copies exactly what the model wrote.

import { useState } from "react";

const COPIED_LABEL_MS = 1500;

// navigator.clipboard, not tauri-plugin-clipboard-manager: the plugin's writeText can crash on macOS
// (tauri-apps/plugins-workspace#3205), and WebKit allows a write from a click in the focused window.
export default function CopyAnswerButton({ answer }: { answer: string }) {
  const [copied, setCopied] = useState(false);

  async function copyAnswer() {
    await navigator.clipboard.writeText(answer);
    setCopied(true);
    setTimeout(() => setCopied(false), COPIED_LABEL_MS);
  }

  return (
    <button className="secondary copy" onClick={() => void copyAnswer()}>
      {copied ? "Copied" : "Copy answer"}
    </button>
  );
}
