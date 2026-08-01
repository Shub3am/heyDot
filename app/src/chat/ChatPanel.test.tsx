import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import type { Channel } from "@tauri-apps/api/core";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { afterEach, expect, test } from "vitest";
import ChatPanel from "./ChatPanel";
import type { AnswerEvent, LocalModelPhase, LocalModelStatus, ScreenShare } from "./ipc";

type AskHandler = (question: string, onEvent: Channel<AnswerEvent>) => Promise<void>;

let invokedCommands: string[] = [];
let statusChannel: Channel<LocalModelStatus> | undefined;

async function renderPanel(onAsk: AskHandler = async () => {}) {
  mockIPC((command, args) => {
    invokedCommands.push(command);
    const namedArgs = args as Record<string, unknown>;
    if (command === "watch_local_model") {
      statusChannel = namedArgs.onStatus as Channel<LocalModelStatus>;
    }
    if (command === "ask_text") {
      return onAsk(namedArgs.question as string, namedArgs.onEvent as Channel<AnswerEvent>);
    }
  });
  render(<ChatPanel />);
  await waitFor(() => expect(statusChannel).toBeDefined());
}

function sendPhase(phase: LocalModelPhase) {
  act(() => statusChannel!.onmessage({ modelName: "Qwen3-VL 4B", downloadBytes: 3_300_000_000, phase }));
}

function typeQuestion(text: string) {
  fireEvent.change(screen.getByLabelText("Question"), { target: { value: text } });
}

const askButton = () => screen.getByRole("button", { name: "Ask" }) as HTMLButtonElement;

function started(screenShare: ScreenShare, leavesDevice = false, host = "127.0.0.1"): AnswerEvent {
  return { event: "started", data: { leavesDevice, host, screen: screenShare } };
}

afterEach(() => {
  cleanup();
  clearMocks();
  invokedCommands = [];
  statusChannel = undefined;
});

test("offers to download a missing model with its size", async () => {
  await renderPanel();
  sendPhase({ kind: "notInstalled" });
  expect(screen.getByText(/3\.3 GB/)).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "Download Qwen3-VL 4B" }));
  await waitFor(() => expect(invokedCommands).toContain("download_local_model"));
});

test("shows download progress as a percent", async () => {
  await renderPanel();
  sendPhase({ kind: "downloading", percent: 42 });
  expect(screen.getByText("Downloading Qwen3-VL 4B: 42%")).toBeTruthy();
});

test("a failed download shows the reason and can be retried", async () => {
  await renderPanel();
  sendPhase({ kind: "downloadFailed", reason: "server answered 503" });
  expect(screen.getByText(/server answered 503/)).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "Try again" }));
  await waitFor(() => expect(invokedCommands).toContain("download_local_model"));
});

test("a stopped server shows the reason and where its log is", async () => {
  await renderPanel();
  sendPhase({ kind: "down", reason: "llama-server stopped 4 times within a minute" });
  expect(screen.getByText(/stopped 4 times within a minute/)).toBeTruthy();
  expect(screen.getByText(/~\/Library\/Logs\/Hey Dot\/llama-server\.log/)).toBeTruthy();
});

test("ask stays disabled until the model is ready", async () => {
  await renderPanel();
  typeQuestion("What is the capital of France?");
  sendPhase({ kind: "starting" });
  expect(askButton().disabled).toBe(true);
  sendPhase({ kind: "ready" });
  expect(askButton().disabled).toBe(false);
});

test("ask stays disabled for a blank question", async () => {
  await renderPanel();
  sendPhase({ kind: "ready" });
  typeQuestion("   ");
  expect(askButton().disabled).toBe(true);
});

test("streams the answer under the question with an on-this-mac badge", async () => {
  await renderPanel(async (_question, onEvent) => {
    onEvent.onmessage(started({ kind: "attached" }));
    onEvent.onmessage({ event: "delta", data: { text: "Par" } });
    onEvent.onmessage({ event: "delta", data: { text: "is" } });
  });
  sendPhase({ kind: "ready" });
  typeQuestion("What is the capital of France?");
  fireEvent.click(askButton());
  expect(await screen.findByText("Paris")).toBeTruthy();
  expect(screen.getByText("On this Mac")).toBeTruthy();
  expect(screen.getByText("What is the capital of France?")).toBeTruthy();
});

test("an answer that leaves the Mac names the host", async () => {
  await renderPanel(async (_question, onEvent) => {
    onEvent.onmessage(started({ kind: "attached" }, true, "api.openai.com"));
  });
  sendPhase({ kind: "ready" });
  typeQuestion("Hi");
  fireEvent.click(askButton());
  expect(await screen.findByText("Sent to api.openai.com")).toBeTruthy();
});

test("a failed answer keeps the text so far and shows the error", async () => {
  await renderPanel(async (_question, onEvent) => {
    onEvent.onmessage({ event: "delta", data: { text: "Par" } });
    throw "could not reach the model server: the connection closed before the answer finished";
  });
  sendPhase({ kind: "ready" });
  typeQuestion("What is the capital of France?");
  fireEvent.click(askButton());
  expect((await screen.findByRole("alert")).textContent).toContain("connection closed");
  expect(screen.getByText("Par")).toBeTruthy();
  typeQuestion("Try again?");
  expect(askButton().disabled).toBe(false);
});

test("sending a question clears the box", async () => {
  await renderPanel();
  sendPhase({ kind: "ready" });
  typeQuestion("What is the capital of France?");
  fireEvent.click(askButton());
  expect((screen.getByLabelText("Question") as HTMLTextAreaElement).value).toBe("");
});

test("ask stays disabled while an answer is streaming", async () => {
  await renderPanel(() => new Promise(() => {}));
  sendPhase({ kind: "ready" });
  typeQuestion("What is the capital of France?");
  fireEvent.click(askButton());
  typeQuestion("And of Spain?");
  expect(askButton().disabled).toBe(true);
});

test("an answer with a screenshot says so", async () => {
  await renderPanel(async (_question, onEvent) => {
    onEvent.onmessage(started({ kind: "attached" }));
  });
  sendPhase({ kind: "ready" });
  typeQuestion("What is on my screen?");
  fireEvent.click(askButton());
  expect(await screen.findByText("With a screenshot of this screen")).toBeTruthy();
});

test("a missing screen permission answers without a screenshot and offers the settings", async () => {
  await renderPanel(async (_question, onEvent) => {
    onEvent.onmessage(started({ kind: "permissionNeeded" }));
  });
  sendPhase({ kind: "ready" });
  typeQuestion("What is on my screen?");
  fireEvent.click(askButton());
  expect((await screen.findByRole("status")).textContent).toContain("needs Screen Recording permission");
  fireEvent.click(screen.getByRole("button", { name: "Open Screen Recording settings" }));
  await waitFor(() => expect(invokedCommands).toContain("open_screen_recording_settings"));
});

test("a failed screenshot says why the answer has none", async () => {
  await renderPanel(async (_question, onEvent) => {
    onEvent.onmessage(started({ kind: "failed", reason: "no display is under the mouse cursor" }));
  });
  sendPhase({ kind: "ready" });
  typeQuestion("What is on my screen?");
  fireEvent.click(askButton());
  expect((await screen.findByRole("status")).textContent).toContain(
    "Answered without a screenshot: no display is under the mouse cursor",
  );
});
