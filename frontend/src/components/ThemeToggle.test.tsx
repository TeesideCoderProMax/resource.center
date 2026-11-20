import { fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, expect, it, vi } from "vitest";
import { ThemeToggle } from "./ThemeToggle";

beforeEach(() => {
  delete document.documentElement.dataset.theme;
  localStorage.clear();
  vi.stubGlobal("matchMedia", () => ({ matches: false, addEventListener() {}, removeEventListener() {} }));
});

it("switches between light and dark and remembers the choice", async () => {
  render(<ThemeToggle />);
  const button = screen.getByRole("button", { name: "Dark mode" });
  expect(button).toHaveAttribute("aria-pressed", "false");
  fireEvent.click(button);
  expect(document.documentElement.dataset.theme).toBe("dark");
  expect(localStorage.getItem("theme")).toBe("dark");
  expect(await screen.findByRole("button", { pressed: true })).toBeInTheDocument();
  fireEvent.click(button);
  expect(document.documentElement.dataset.theme).toBe("light");
});
