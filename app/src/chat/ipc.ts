// The typed boundary between the UI and the Rust commands in src-tauri/src/commands.rs and history_commands.rs.
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

export type ScreenShare =
  | { kind: "attached" }
  | { kind: "permissionNeeded" }
  | { kind: "failed"; reason: string };

export type AnswerEvent =
  | {
      event: "started";
      data: { leavesDevice: boolean; host: string; screen: ScreenShare; forgotEarlierTurns: boolean };
    }
  | { event: "delta"; data: { text: string } };

export type HistoryStatus = { kind: "off" } | { kind: "on"; saving: boolean } | { kind: "unavailable"; reason: string };

export type SavedChatRow = { id: number; title: string; updatedAtMs: number; turnCount: number };

export type SavedTurnRow = { question: string; hadScreenshot: boolean; answer: string };

export function watchLocalModel(onStatus: (status: LocalModelStatus) => void): Promise<void> {
  return invoke("watch_local_model", { onStatus: new Channel(onStatus) });
}

export function downloadLocalModel(): Promise<void> {
  return invoke("download_local_model");
}

/** Resolves when the answer is complete or a newer question, Stop or New chat stopped it; rejects with the error text when it fails. */
export function askText(question: string, onEvent: (event: AnswerEvent) => void): Promise<void> {
  return invoke("ask_text", { question, onEvent: new Channel(onEvent) });
}

export function openScreenRecordingSettings(): Promise<void> {
  return invoke("open_screen_recording_settings");
}

/** Stops the running answer; the next question still follows up on it. */
export function stopAnswer(): Promise<void> {
  return invoke("stop_answer");
}

/** Stops the running answer; the next question starts a new chat. */
export function newChat(): Promise<void> {
  return invoke("new_chat");
}

export function historyStatus(): Promise<HistoryStatus> {
  return invoke("history_status");
}

/** Creates the encrypted saved chats file on first use; saving starts on. */
export function turnOnHistory(): Promise<HistoryStatus> {
  return invoke("turn_on_history");
}

/** Pausing or resuming ends the chat being saved, so the next question starts a new one. */
export function setHistorySaving(saving: boolean): Promise<HistoryStatus> {
  return invoke("set_history_saving", { saving });
}

export function listSavedChats(search: string): Promise<SavedChatRow[]> {
  return invoke("list_saved_chats", { search });
}

/** Resolves with the chat's turns once the next question follows up on them; rejects when the chat is gone. */
export function openSavedChat(chatId: number): Promise<SavedTurnRow[]> {
  return invoke("open_saved_chat", { chatId });
}

export function deleteSavedChat(chatId: number): Promise<void> {
  return invoke("delete_saved_chat", { chatId });
}

export function deleteAllSavedChats(): Promise<void> {
  return invoke("delete_all_saved_chats");
}
