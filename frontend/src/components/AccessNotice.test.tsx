import { render, screen } from "@testing-library/react";
import { expect, test } from "vitest";
import { AccessNotice } from "./AccessNotice";

test("non-staff see a 403 staff-only alert", () => {
  render(<AccessNotice access="forbidden" returnTo="/staff" />);
  expect(screen.getByRole("heading")).toHaveTextContent("403 — staff only");
  expect(screen.getByRole("alert")).toBeInTheDocument();
});

test("signed-out users get a sign-in link that returns to the page", () => {
  render(<AccessNotice access="sign-in" returnTo="/reservations" />);
  expect(screen.getByRole("link", { name: "Sign in" })).toHaveAttribute("href", "/auth/login?returnTo=%2Freservations");
});
