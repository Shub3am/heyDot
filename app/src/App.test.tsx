import { render, screen } from "@testing-library/react";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { afterEach, beforeEach, expect, test } from "vitest";
import App from "./App";

beforeEach(() => mockIPC(() => {}));
afterEach(() => clearMocks());

test("opens on the Chat page", () => {
  render(<App />);
  expect(screen.getByRole("heading", { name: "Chat" })).toBeTruthy();
});
