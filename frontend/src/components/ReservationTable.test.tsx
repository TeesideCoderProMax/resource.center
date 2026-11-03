import { render, screen } from "@testing-library/react";
import { expect, test } from "vitest";
import type { ReservationRow } from "@/lib/loan";
import { isOverdue, ReservationTable } from "./ReservationTable";

const row = (o: Partial<ReservationRow>): ReservationRow => ({
  reservationId: "r", modelId: "m", unitId: "u", studentSub: "s", studentEmail: "e", status: "COLLECTED",
  reservedAt: "2026-01-01T00:00:00Z", dueAt: "2026-01-03T00:00:00Z", collectedAt: null, returnedAt: null, ...o,
});
const now = Date.parse("2026-01-04T00:00:00Z");

test("overdue means due date passed and not returned", () => {
  expect(isOverdue(row({}), now)).toBe(true);
  expect(isOverdue(row({ status: "RETURNED" }), now)).toBe(false);
  expect(isOverdue(row({ dueAt: "2026-01-05T00:00:00Z" }), now)).toBe(false);
});

test("table shows status badges and highlights overdue rows", () => {
  render(<ReservationTable rows={[row({ reservationId: "a", dueAt: "2000-01-01T00:00:00Z" }), row({ reservationId: "b", dueAt: "2999-01-01T00:00:00Z", status: "RESERVED" })]} />);
  expect(screen.getAllByText("Overdue")).toHaveLength(1);
  expect(screen.getByText("RESERVED")).toHaveClass("status", "s-reserved");
});
