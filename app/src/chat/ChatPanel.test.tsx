import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import type { Channel } from "@tauri-apps/api/core";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { afterEach, expect, test, vi } from "vitest";
import useLocalModelStatus from "../localModel/useLocalModelStatus";
import ChatPanel from "./ChatPanel";
import type { AnswerEvent, LocalModelPhase, LocalModelStatus, ScreenShare } from "./ipc";

type AskHandler = (question: string, onEvent: Channel<AnswerEvent>) => Promise<void>;

let invokedCommands: string[] = [];
let statusChannel: Channel<LocalModelStatus> | undefined;

function ChatPanelWithModel() {
  return <ChatPanel status={useLocalModelStatus()} isVisible />;
}

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
  render(<ChatPanelWithModel />);
  await waitFor(() => expect(statusChannel).toBeDefined());
}

function sendPhase(phase: LocalModelPhase) {
  act(() => statusChannel!.onmessage({ modelName: "Qwen3-VL 4B", downloadBytes: 3_300_000_000, phase }));
}

function typeQuestion(text: string) {
  fireEvent.change(screen.getByLabelText("Question"), { target: { value: text } });
}

const askButton = () => screen.getByRole("button", { name: "Ask" }) as HTMLButtonElement;

function started(
  screenShare: ScreenShare,
  leavesDevice = false,
  host = "127.0.0.1",
  forgotEarlierTurns = false,
): AnswerEvent {
  return { event: "started", data: { leavesDevice, host, screen: screenShare, forgotEarlierTurns } };
}

const stopButton = () => screen.getByRole("button", { name: "Stop" }) as HTMLButtonElement;

afterEach(() => {
  cleanup();
  clearMocks();
  invokedCommands = [];
  statusChannel = undefined;
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

test("asking while an answer streams sends the new question and keeps the earlier one", async () => {
  const askedQuestions: string[] = [];
  await renderPanel((question, onEvent) => {
    askedQuestions.push(question);
    if (question === "What is the capital of France?") {
      onEvent.onmessage({ event: "delta", data: { text: "Par" } });
      return new Promise(() => {});
    }
    return Promise.resolve();
  });
  sendPhase({ kind: "ready" });
  typeQuestion("What is the capital of France?");
  fireEvent.click(askButton());
  expect(await screen.findByText("Par")).toBeTruthy();
  typeQuestion("And of Spain?");
  expect(askButton().disabled).toBe(false);
  fireEvent.click(askButton());
  await waitFor(() => expect(askedQuestions).toEqual(["What is the capital of France?", "And of Spain?"]));
  expect(screen.getByText("What is the capital of France?")).toBeTruthy();
  expect(screen.getByText("And of Spain?")).toBeTruthy();
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

test("a follow-up appears under the earlier answer", async () => {
  await renderPanel(async (question, onEvent) => {
    onEvent.onmessage(started({ kind: "attached" }));
    onEvent.onmessage({ event: "delta", data: { text: `Answer to ${question}` } });
  });
  sendPhase({ kind: "ready" });
  typeQuestion("What city is this?");
  fireEvent.click(askButton());
  expect(await screen.findByText("Answer to What city is this?")).toBeTruthy();
  typeQuestion("How many people live there?");
  fireEvent.click(askButton());
  expect(await screen.findByText("Answer to How many people live there?")).toBeTruthy();
  const turns = screen.getAllByRole("article");
  expect(turns).toHaveLength(2);
  expect(turns[0].textContent).toContain("Answer to What city is this?");
  expect(turns[1].textContent).toContain("Answer to How many people live there?");
});

test("a question Hey Dot answers after ten idle minutes hides the turns it forgot", async () => {
  await renderPanel(async (question, onEvent) => {
    const forgotEarlierTurns = question === "What is on my screen now?";
    onEvent.onmessage(started({ kind: "attached" }, false, "127.0.0.1", forgotEarlierTurns));
    onEvent.onmessage({ event: "delta", data: { text: `Answer to ${question}` } });
  });
  sendPhase({ kind: "ready" });
  typeQuestion("What city is this?");
  fireEvent.click(askButton());
  expect(await screen.findByText("Answer to What city is this?")).toBeTruthy();
  typeQuestion("What is on my screen now?");
  fireEvent.click(askButton());
  expect(await screen.findByText("Answer to What is on my screen now?")).toBeTruthy();
  expect(screen.queryByText("What city is this?")).toBeNull();
  expect(screen.getAllByRole("article")).toHaveLength(1);
});

test("Enter asks the typed question", async () => {
  await renderPanel(async (question, onEvent) => {
    onEvent.onmessage(started({ kind: "attached" }));
    onEvent.onmessage({ event: "delta", data: { text: `Answer to ${question}` } });
  });
  sendPhase({ kind: "ready" });
  typeQuestion("What city is this?");
  fireEvent.keyDown(screen.getByLabelText("Question"), { key: "Enter" });
  expect(await screen.findByText("Answer to What city is this?")).toBeTruthy();
});

test("Shift+Enter starts a new line instead of asking", async () => {
  await renderPanel();
  sendPhase({ kind: "ready" });
  typeQuestion("What city is this?");
  fireEvent.keyDown(screen.getByLabelText("Question"), { key: "Enter", shiftKey: true });
  expect(invokedCommands).not.toContain("ask_text");
});

test("Enter while an input method is composing does not ask", async () => {
  await renderPanel();
  sendPhase({ kind: "ready" });
  typeQuestion("東京");
  fireEvent.keyDown(screen.getByLabelText("Question"), { key: "Enter", isComposing: true });
  expect(invokedCommands).not.toContain("ask_text");
});

test("Enter on an empty question does not ask", async () => {
  await renderPanel();
  sendPhase({ kind: "ready" });
  typeQuestion("   ");
  fireEvent.keyDown(screen.getByLabelText("Question"), { key: "Enter" });
  expect(invokedCommands).not.toContain("ask_text");
});

test("renders the answer as markdown with highlighted code", async () => {
  await renderPanel(async (_question, onEvent) => {
    onEvent.onmessage(started({ kind: "attached" }));
    onEvent.onmessage({ event: "delta", data: { text: "It is **Paris**.\n\n```js\nconst city = " } });
    onEvent.onmessage({ event: "delta", data: { text: '"Paris";\n```\n' } });
  });
  sendPhase({ kind: "ready" });
  typeQuestion("What is the capital of France?");
  fireEvent.click(askButton());
  expect((await screen.findByText("Paris", { selector: "strong" })).tagName).toBe("STRONG");
  expect(screen.getByText("const").className).toContain("hljs-keyword");
});

test("shows HTML in an answer as text instead of running it", async () => {
  await renderPanel(async (_question, onEvent) => {
    onEvent.onmessage(started({ kind: "attached" }));
    onEvent.onmessage({ event: "delta", data: { text: 'Hi <img src="x" onerror="alert(1)">' } });
  });
  sendPhase({ kind: "ready" });
  typeQuestion("Say hi");
  fireEvent.click(askButton());
  await screen.findByText(/Hi/);
  expect(document.querySelector(".answer img")).toBeNull();
});

test("renders a markdown table in the answer as a table", async () => {
  await renderPanel(async (_question, onEvent) => {
    onEvent.onmessage(started({ kind: "attached" }));
    onEvent.onmessage({ event: "delta", data: { text: "| City | Country |\n| --- | --- |\n| Paris | France |\n" } });
  });
  sendPhase({ kind: "ready" });
  typeQuestion("List capitals");
  fireEvent.click(askButton());
  expect(await screen.findByRole("table")).toBeTruthy();
  expect(screen.getByRole("cell", { name: "Paris" })).toBeTruthy();
});

test("copies the answer as markdown", async () => {
  const writeText = vi.fn(async (_text: string) => {});
  Object.defineProperty(navigator, "clipboard", { value: { writeText }, configurable: true });
  await renderPanel(async (_question, onEvent) => {
    onEvent.onmessage(started({ kind: "attached" }));
    onEvent.onmessage({ event: "delta", data: { text: "It is **Paris**." } });
  });
  sendPhase({ kind: "ready" });
  typeQuestion("What is the capital of France?");
  fireEvent.click(askButton());
  fireEvent.click(await screen.findByRole("button", { name: "Copy answer" }));
  expect(writeText).toHaveBeenCalledWith("It is **Paris**.");
  expect(await screen.findByRole("button", { name: "Copied" })).toBeTruthy();
});

test("offers no copy button before any answer text arrives", async () => {
  await renderPanel(async (_question, onEvent) => {
    onEvent.onmessage(started({ kind: "attached" }));
  });
  sendPhase({ kind: "ready" });
  typeQuestion("What is the capital of France?");
  fireEvent.click(askButton());
  await screen.findByText("On this Mac");
  expect(screen.queryByRole("button", { name: "Copy answer" })).toBeNull();
});

test("Stop is disabled while no answer is streaming", async () => {
  await renderPanel();
  sendPhase({ kind: "ready" });
  expect(stopButton().disabled).toBe(true);
});

test("Stop ends a streaming answer and keeps its text so far", async () => {
  await renderPanel((_question, onEvent) => {
    onEvent.onmessage({ event: "delta", data: { text: "Par" } });
    return new Promise(() => {});
  });
  sendPhase({ kind: "ready" });
  typeQuestion("What is the capital of France?");
  fireEvent.click(askButton());
  expect(await screen.findByText("Par")).toBeTruthy();
  expect(stopButton().disabled).toBe(false);
  fireEvent.click(stopButton());
  await waitFor(() => expect(invokedCommands).toContain("stop_answer"));
  expect(screen.getByText("Par")).toBeTruthy();
});

test("Stop is disabled again once the answer finishes", async () => {
  await renderPanel(async (_question, onEvent) => {
    onEvent.onmessage({ event: "delta", data: { text: "Paris" } });
  });
  sendPhase({ kind: "ready" });
  typeQuestion("What is the capital of France?");
  fireEvent.click(askButton());
  expect(await screen.findByText("Paris")).toBeTruthy();
  await waitFor(() => expect(stopButton().disabled).toBe(true));
});

test("an earlier answer ending does not disable Stop for the newer one", async () => {
  let finishFirstAnswer: () => void = () => {};
  await renderPanel((question, onEvent) => {
    if (question === "What is the capital of France?") {
      return new Promise((resolve) => (finishFirstAnswer = () => resolve()));
    }
    onEvent.onmessage({ event: "delta", data: { text: "Madrid" } });
    return new Promise(() => {});
  });
  sendPhase({ kind: "ready" });
  typeQuestion("What is the capital of France?");
  fireEvent.click(askButton());
  typeQuestion("And of Spain?");
  fireEvent.click(askButton());
  expect(await screen.findByText("Madrid")).toBeTruthy();
  await act(async () => finishFirstAnswer());
  expect(stopButton().disabled).toBe(false);
});
