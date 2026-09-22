import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { afterEach, expect, test, vi } from "vitest";
import type { HistoryStatus, SavedChatRow, SavedTurnRow } from "../chat/ipc";
import HistoryPage from "./HistoryPage";

type FakeBackend = { status: HistoryStatus; chats: SavedChatRow[]; turns: SavedTurnRow[] };

let invokedCommands: { command: string; args: Record<string, unknown> }[] = [];

function chat(id: number, title: string, turnCount = 1): SavedChatRow {
  return { id, title, updatedAtMs: Date.UTC(2026, 8, 20, 10, 30), turnCount };
}

function renderPage(backend: FakeBackend, onOpenChat = vi.fn()) {
  mockIPC((command, args) => {
    const namedArgs = args as Record<string, unknown>;
    invokedCommands.push({ command, args: namedArgs });
    switch (command) {
      case "history_status":
        return backend.status;
      case "turn_on_history":
        backend.status = { kind: "on", saving: true };
        return backend.status;
      case "set_history_saving":
        backend.status = { kind: "on", saving: namedArgs.saving as boolean };
        return backend.status;
      case "list_saved_chats":
        return backend.chats.filter((row) => row.title.includes(namedArgs.search as string));
      case "open_saved_chat":
        if (!backend.chats.some((row) => row.id === namedArgs.chatId)) {
          throw "This chat no longer exists.";
        }
        return backend.turns;
      case "delete_saved_chat":
        backend.chats = backend.chats.filter((row) => row.id !== namedArgs.chatId);
        return;
      case "delete_all_saved_chats":
        backend.chats = [];
        return;
    }
  });
  render(<HistoryPage onOpenChat={onOpenChat} />);
  return onOpenChat;
}

const commandNames = () => invokedCommands.map((invoked) => invoked.command);

afterEach(() => {
  cleanup();
  clearMocks();
  invokedCommands = [];
});

test("while off it explains saving and offers to turn it on", async () => {
  renderPage({ status: { kind: "off" }, chats: [], turns: [] });
  expect(await screen.findByRole("button", { name: "Turn on saving" })).toBeTruthy();
  expect(screen.getByText(/stored encrypted on this Mac/)).toBeTruthy();
  expect(screen.getByText(/A chat lasts until you click New chat/)).toBeTruthy();
});

test("turning saving on shows the saved chats", async () => {
  renderPage({ status: { kind: "off" }, chats: [], turns: [] });
  fireEvent.click(await screen.findByRole("button", { name: "Turn on saving" }));
  expect(await screen.findByLabelText("Search saved chats")).toBeTruthy();
  expect(screen.getByText(/No saved chats yet/)).toBeTruthy();
  expect(commandNames()).toContain("turn_on_history");
});

test("lists each saved chat with its title, size and visible Open and Delete buttons", async () => {
  renderPage({ status: { kind: "on", saving: true }, chats: [chat(1, "What city is this?", 3)], turns: [] });
  const row = await screen.findByRole("listitem");
  expect(row.textContent).toContain("What city is this?");
  expect(row.textContent).toContain("3 questions");
  expect(screen.getByRole("button", { name: "Open" })).toBeTruthy();
  expect(screen.getByRole("button", { name: "Delete" })).toBeTruthy();
});

test("searching lists only the chats that match", async () => {
  renderPage({ status: { kind: "on", saving: true }, chats: [chat(1, "Figma layers"), chat(2, "Weather")], turns: [] });
  expect(await screen.findAllByRole("listitem")).toHaveLength(2);
  fireEvent.change(screen.getByLabelText("Search saved chats"), { target: { value: "Figma" } });
  await waitFor(() => expect(screen.getAllByRole("listitem")).toHaveLength(1));
  expect(screen.getByRole("listitem").textContent).toContain("Figma layers");
});

test("a search with no match says so", async () => {
  renderPage({ status: { kind: "on", saving: true }, chats: [chat(1, "Weather")], turns: [] });
  await screen.findByRole("listitem");
  fireEvent.change(screen.getByLabelText("Search saved chats"), { target: { value: "Figma" } });
  expect(await screen.findByText(/No saved chats match/)).toBeTruthy();
});

test("Open hands the chat's turns to the Chat page", async () => {
  const turns = [{ question: "What city is this?", hadScreenshot: true, answer: "Paris." }];
  const onOpenChat = renderPage({ status: { kind: "on", saving: true }, chats: [chat(7, "What city is this?")], turns });
  fireEvent.click(await screen.findByRole("button", { name: "Open" }));
  await waitFor(() => expect(onOpenChat).toHaveBeenCalledWith(turns));
  expect(invokedCommands).toContainEqual({ command: "open_saved_chat", args: { chatId: 7 } });
});

test("opening a chat that is gone says so and refreshes the list", async () => {
  const backend: FakeBackend = { status: { kind: "on", saving: true }, chats: [chat(7, "Gone")], turns: [] };
  const onOpenChat = renderPage(backend);
  await screen.findByRole("listitem");
  backend.chats = [];
  fireEvent.click(screen.getByRole("button", { name: "Open" }));
  expect((await screen.findByRole("alert")).textContent).toContain("This chat no longer exists.");
  await waitFor(() => expect(screen.queryByRole("listitem")).toBeNull());
  expect(onOpenChat).not.toHaveBeenCalled();
});

test("Delete removes that chat only", async () => {
  renderPage({ status: { kind: "on", saving: true }, chats: [chat(1, "Delete me"), chat(2, "Keep me")], turns: [] });
  const rows = await screen.findAllByRole("listitem");
  fireEvent.click(within(rows[0]).getByRole("button", { name: "Delete" }));
  await waitFor(() => expect(screen.getAllByRole("listitem")).toHaveLength(1));
  expect(screen.getByRole("listitem").textContent).toContain("Keep me");
});

test("Delete all asks first and Cancel keeps every chat", async () => {
  renderPage({ status: { kind: "on", saving: true }, chats: [chat(1, "One"), chat(2, "Two")], turns: [] });
  await screen.findAllByRole("listitem");
  fireEvent.click(screen.getByRole("button", { name: "Delete all" }));
  fireEvent.click(screen.getByRole("button", { name: "Cancel" }));
  expect(screen.getByRole("button", { name: "Delete all" })).toBeTruthy();
  expect(screen.getAllByRole("listitem")).toHaveLength(2);
  expect(commandNames()).not.toContain("delete_all_saved_chats");
});

test("confirming Delete all removes every chat", async () => {
  renderPage({ status: { kind: "on", saving: true }, chats: [chat(1, "One"), chat(2, "Two")], turns: [] });
  await screen.findAllByRole("listitem");
  fireEvent.click(screen.getByRole("button", { name: "Delete all" }));
  fireEvent.click(screen.getByRole("button", { name: "Delete all chats" }));
  expect(await screen.findByText(/No saved chats yet/)).toBeTruthy();
});

test("Pause saving says new chats are not saved and offers Resume saving", async () => {
  renderPage({ status: { kind: "on", saving: true }, chats: [], turns: [] });
  fireEvent.click(await screen.findByRole("button", { name: "Pause saving" }));
  expect(await screen.findByRole("button", { name: "Resume saving" })).toBeTruthy();
  expect(screen.getByText(/Saving is paused/)).toBeTruthy();
  expect(invokedCommands).toContainEqual({ command: "set_history_saving", args: { saving: false } });
});

test("unavailable saved chats show the reason with no buttons", async () => {
  renderPage({ status: { kind: "unavailable", reason: "the key is missing" }, chats: [], turns: [] });
  expect((await screen.findByRole("alert")).textContent).toContain("the key is missing");
  expect(screen.queryAllByRole("button")).toHaveLength(0);
});
