// The typed boundary between the chat panel and the Rust commands in src-tauri/src/commands.rs.
// Must not hold UI state; the shapes here mirror the Rust serde output exactly.

import { Channel, invoke } from "@tauri-apps/api/core";

export type LocalModelPhase =
  | { kind: "notInstalled" }
  | { kind: "downloading"; percent: number }
  | { kind: "downloadFailed"; reason: string }
  | { kind: "starting" }
  | { kind: "ready" }
  | { kind: "down"; reason: string };

export type LocalModelStatus = {
  modelName: string;
  downloadBytes: number;
  phase: LocalModelPhase;
};

export type AnswerEvent =
  | { event: "started"; data: { leavesDevice: boolean; host: string } }
  | { event: "delta"; data: { text: string } };

export function watchLocalModel(onStatus: (status: LocalModelStatus) => void): Promise<void> {
  return invoke("watch_local_model", { onStatus: new Channel(onStatus) });
}

export function downloadLocalModel(): Promise<void> {
  return invoke("download_local_model");
}

/** Rejects with the error text when the answer fails part way. */
export function askText(question: string, onEvent: (event: AnswerEvent) => void): Promise<void> {
  return invoke("ask_text", { question, onEvent: new Channel(onEvent) });
}
