import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import type { Channel } from "@tauri-apps/api/core";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { afterEach, expect, test } from "vitest";
import type { LocalModelPhase, LocalModelStatus } from "../chat/ipc";
import ModelStatus from "./ModelStatus";
import useLocalModelStatus from "./useLocalModelStatus";

let invokedCommands: string[] = [];
let statusChannel: Channel<LocalModelStatus> | undefined;

function WatchedModelStatus() {
  return <ModelStatus status={useLocalModelStatus()} />;
}

async function renderStatus() {
  mockIPC((command, args) => {
    invokedCommands.push(command);
    if (command === "watch_local_model") {
      statusChannel = (args as Record<string, unknown>).onStatus as Channel<LocalModelStatus>;
    }
  });
  render(<WatchedModelStatus />);
  await waitFor(() => expect(statusChannel).toBeDefined());
}

function sendPhase(phase: LocalModelPhase) {
  act(() => statusChannel!.onmessage({ modelName: "Qwen3-VL 4B", downloadBytes: 3_300_000_000, phase }));
}

afterEach(() => {
  cleanup();
  clearMocks();
  invokedCommands = [];
  statusChannel = undefined;
});

test("says it is checking before the first status arrives", async () => {
  await renderStatus();
  expect(screen.getByText("Checking the local model...")).toBeTruthy();
});

test("offers to download a missing model with its size", async () => {
  await renderStatus();
  sendPhase({ kind: "notInstalled" });
  expect(screen.getByText(/3\.3 GB/)).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "Download Qwen3-VL 4B" }));
  await waitFor(() => expect(invokedCommands).toContain("download_local_model"));
});

test("shows download progress as a percent", async () => {
  await renderStatus();
  sendPhase({ kind: "downloading", percent: 42 });
  expect(screen.getByText("Downloading Qwen3-VL 4B: 42%")).toBeTruthy();
});

test("a failed download shows the reason and can be retried", async () => {
  await renderStatus();
  sendPhase({ kind: "downloadFailed", reason: "server answered 503" });
  expect(screen.getByText(/server answered 503/)).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "Try again" }));
  await waitFor(() => expect(invokedCommands).toContain("download_local_model"));
});

test("a stopped server shows the reason and where its log is", async () => {
  await renderStatus();
  sendPhase({ kind: "down", reason: "llama-server stopped 4 times within a minute" });
  expect(screen.getByText(/stopped 4 times within a minute/)).toBeTruthy();
  expect(screen.getByText(/~\/Library\/Logs\/Hey Dot\/llama-server\.log/)).toBeTruthy();
});

test("a ready model shows the ready dot", async () => {
  await renderStatus();
  sendPhase({ kind: "ready" });
  const sentence = screen.getByText("Qwen3-VL 4B is ready.");
  expect(sentence.closest(".model-status")!.getAttribute("data-dot")).toBe("ready");
});
