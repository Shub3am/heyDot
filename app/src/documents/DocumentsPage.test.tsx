import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, expect, test } from "vitest";
import DocumentsPage from "./DocumentsPage";

afterEach(cleanup);

test("says what is coming and what answers use today, with no buttons", () => {
  render(<DocumentsPage />);
  expect(screen.getByText(/Drop in PDFs, notes and folders/)).toBeTruthy();
  expect(screen.getByText("Hey Dot answers from your screen and the chat only.")).toBeTruthy();
  expect(screen.queryAllByRole("button")).toHaveLength(0);
});
