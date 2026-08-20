import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, expect, test } from "vitest";
import HistoryPage from "./HistoryPage";

afterEach(cleanup);

test("says what is coming and how chats last today, with no buttons", () => {
  render(<HistoryPage />);
  expect(screen.getByText(/Saved chats you can search, reopen and delete/)).toBeTruthy();
  expect(screen.getByText(/A chat lasts until you click New chat/)).toBeTruthy();
  expect(screen.queryAllByRole("button")).toHaveLength(0);
});
