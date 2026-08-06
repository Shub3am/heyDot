import { render, screen } from "@testing-library/react";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { afterEach, beforeEach, expect, test } from "vitest";
import App from "./App";

beforeEach(() => mockIPC(() => {}));
afterEach(() => clearMocks());

test("renders the app name", () => {
  render(<App />);
  expect(screen.getByRole("heading", { name: "Hey Dot" })).toBeTruthy();
});
