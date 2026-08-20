import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { afterEach, expect, test } from "vitest";
import SettingsPage from "./SettingsPage";

afterEach(() => {
  cleanup();
  clearMocks();
});

test("says what is coming and offers only the Screen Recording settings", async () => {
  const invokedCommands: string[] = [];
  mockIPC((command) => {
    invokedCommands.push(command);
  });
  render(<SettingsPage />);
  expect(screen.getByText(/Hotkeys, voice, screenshot size, model choice and launch at login/)).toBeTruthy();
  const buttons = screen.getAllByRole("button");
  expect(buttons.map((button) => button.textContent)).toEqual(["Open Screen Recording settings"]);
  fireEvent.click(buttons[0]);
  await waitFor(() => expect(invokedCommands).toContain("open_screen_recording_settings"));
});
