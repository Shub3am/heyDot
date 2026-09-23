import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import type { Channel } from "@tauri-apps/api/core";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { afterEach, expect, test, vi } from "vitest";
import App from "./App";
import type { AnswerEvent, LocalModelStatus, SavedTurnRow } from "./chat/ipc";

type AskHandler = (onEvent: Channel<AnswerEvent>) => Promise<void>;

let invokedCommands: string[] = [];
let statusChannel: Channel<LocalModelStatus> | undefined;

const savedTurns: SavedTurnRow[] = [{ question: "What city is this?", hadScreenshot: false, answer: "Paris." }];

async function renderApp(onAsk: AskHandler = async () => {}) {
  mockIPC((command, args) => {
    invokedCommands.push(command);
    const namedArgs = args as Record<string, unknown>;
    if (command === "watch_local_model") {
      statusChannel = namedArgs.onStatus as Channel<LocalModelStatus>;
    }
    if (command === "ask_text") {
      return onAsk(namedArgs.onEvent as Channel<AnswerEvent>);
    }
    if (command === "history_status") {
      return { kind: "on", saving: true };
    }
    if (command === "list_saved_chats") {
      return [{ id: 7, title: "What city is this?", updatedAtMs: 0, turnCount: 1 }];
    }
    if (command === "open_saved_chat") {
      return savedTurns;
    }
  });
  render(<App />);
  await waitFor(() => expect(statusChannel).toBeDefined());
  act(() => statusChannel!.onmessage({ modelName: "Qwen3-VL 4B", downloadBytes: 3_300_000_000, phase: { kind: "ready" } }));
}

function ask(question: string) {
  fireEvent.change(screen.getByLabelText("Question"), { target: { value: question } });
  fireEvent.click(screen.getByRole("button", { name: "Ask" }));
}

/** The answer's channel stays open, as if the model were still streaming. */
async function askWithAnswerStillStreaming(question: string) {
  let answerChannel: Channel<AnswerEvent> | undefined;
  await renderApp((onEvent) => {
    answerChannel = onEvent;
    return new Promise(() => {});
  });
  ask(question);
  await waitFor(() => expect(answerChannel).toBeDefined());
  return answerChannel!;
}

const pageButton = (label: string) => screen.getByRole("button", { name: label });

afterEach(() => {
  vi.restoreAllMocks();
  cleanup();
  clearMocks();
  invokedCommands = [];
  statusChannel = undefined;
});

test("opens on the Chat page", async () => {
  await renderApp();
  expect(screen.getByRole("heading", { name: "Chat" })).toBeTruthy();
  expect(pageButton("Chat").getAttribute("aria-current")).toBe("page");
});

test("each page button shows its page and marks itself as the current page", async () => {
  await renderApp();
  for (const label of ["History", "Documents", "Settings", "Chat"]) {
    fireEvent.click(pageButton(label));
    expect(screen.getByRole("heading", { level: 1, name: label })).toBeTruthy();
    for (const other of ["Chat", "History", "Documents", "Settings"]) {
      expect(pageButton(other).getAttribute("aria-current")).toBe(other === label ? "page" : null);
    }
  }
});

test("the model status shows in the sidebar on every page", async () => {
  await renderApp();
  fireEvent.click(pageButton("Documents"));
  expect(screen.getByText("Qwen3-VL 4B is ready.")).toBeTruthy();
});

test("turns in Chat survive a switch to History and back", async () => {
  await renderApp(async (onEvent) => {
    onEvent.onmessage({ event: "delta", data: { text: "Paris" } });
  });
  ask("What is the capital of France?");
  expect(await screen.findByText("Paris")).toBeTruthy();
  fireEvent.click(pageButton("History"));
  expect(screen.queryByRole("article")).toBeNull();
  fireEvent.click(pageButton("Chat"));
  expect(screen.getByRole("article").textContent).toContain("Paris");
});

test("an answer that grows while Chat is hidden is scrolled to its end when Chat opens again", async () => {
  // jsdom has no layout. shell.css keeps the hidden Chat page's box, so its conversation still has a height.
  vi.spyOn(HTMLElement.prototype, "scrollHeight", "get").mockImplementation(function (this: HTMLElement) {
    return this.textContent!.length * 10;
  });
  const answerChannel = await askWithAnswerStillStreaming("What is the capital of France?");
  fireEvent.click(pageButton("History"));
  act(() => answerChannel.onmessage({ event: "delta", data: { text: "Paris is the capital of France." } }));
  fireEvent.click(pageButton("Chat"));
  const conversation = document.querySelector<HTMLElement>(".conversation")!;
  expect(conversation.scrollTop).toBe(conversation.scrollHeight);
});

test("New chat from another page shows an empty Chat and tells Hey Dot to forget it", async () => {
  await renderApp(async (onEvent) => {
    onEvent.onmessage({ event: "delta", data: { text: "Paris" } });
  });
  ask("What is the capital of France?");
  expect(await screen.findByText("Paris")).toBeTruthy();
  fireEvent.click(pageButton("History"));
  fireEvent.click(pageButton("New chat"));
  expect(screen.getByRole("heading", { name: "Chat" })).toBeTruthy();
  expect(screen.queryByRole("article")).toBeNull();
  await waitFor(() => expect(invokedCommands).toContain("new_chat"));
});

test("an answer still streaming when New chat is clicked does not come back", async () => {
  const answerChannel = await askWithAnswerStillStreaming("What is the capital of France?");
  fireEvent.click(pageButton("New chat"));
  act(() => answerChannel.onmessage({ event: "delta", data: { text: "Par" } }));
  expect(screen.queryByText("Par")).toBeNull();
  expect(screen.queryByRole("article")).toBeNull();
});

test("opening a saved chat shows its turns on the Chat page", async () => {
  await renderApp();
  fireEvent.click(pageButton("History"));
  fireEvent.click(await screen.findByRole("button", { name: "Open" }));
  expect(await screen.findByRole("heading", { name: "Chat" })).toBeTruthy();
  expect(screen.getByRole("article").textContent).toContain("Paris.");
});

test("New chat after opening a saved chat shows an empty Chat", async () => {
  await renderApp();
  fireEvent.click(pageButton("History"));
  fireEvent.click(await screen.findByRole("button", { name: "Open" }));
  await screen.findByRole("article");
  fireEvent.click(pageButton("New chat"));
  expect(screen.queryByRole("article")).toBeNull();
});

test("the top of the sidebar is a window drag strip with no controls in it", async () => {
  await renderApp();
  const dragStrip = screen.getByRole("navigation", { name: "Pages" }).firstElementChild!;
  expect(dragStrip.hasAttribute("data-tauri-drag-region")).toBe(true);
  expect(dragStrip.children.length).toBe(0);
});
