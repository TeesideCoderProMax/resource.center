import { fireEvent, render, screen, within } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";
import type { ReservationRow } from "@/lib/loan";
import { fakeFetch } from "@/test/fake-fetch";
import { StaffDesk } from "./StaffDesk";

afterEach(() => vi.unstubAllGlobals());

const ID = "0f8fad5b-d9cb-469f-a165-70867728950e";
const row = (o: Partial<ReservationRow> = {}): ReservationRow => ({
  reservationId: ID, modelId: "dell", unitId: "dell-u1", studentSub: "s", studentEmail: "s1@campus.test",
  status: "RESERVED", reservedAt: "2099-01-01T00:00:00Z", dueAt: "2099-01-03T00:00:00Z", collectedAt: null, returnedAt: null, ...o,
});

test("collect needs a confirm step, then shows the timestamp and reloads", async () => {
  const fetch = fakeFetch({
    "GET reservations": [{ status: 200, body: [row()] }, { status: 200, body: [row({ status: "COLLECTED", collectedAt: "2099-01-01T01:00:00Z" })] }],
    [`POST reservations/${ID}/collect`]: { status: 200, body: { collectedAt: "2099-01-01T01:00:00Z" } },
  });
  render(<StaffDesk />);
  fireEvent.click(await screen.findByRole("button", { name: "Mark collected" }));
  expect(fetch).toHaveBeenCalledTimes(1);
  fireEvent.click(screen.getByRole("button", { name: "Confirm collect" }));
  expect(await screen.findByRole("status")).toHaveTextContent(/Collected dell-u1 at .* for s1@campus\.test\. .*Confirmation email queued/);
  expect(await screen.findByRole("button", { name: "Mark returned" })).toBeInTheDocument();
  expect(fetch).toHaveBeenCalledTimes(3);
});

test("409 InvalidTransition explains and refreshes the row", async () => {
  fakeFetch({
    "GET reservations": [{ status: 200, body: [row()] }, { status: 200, body: [row({ status: "COLLECTED" })] }],
    [`POST reservations/${ID}/collect`]: { status: 409, body: { error: "InvalidTransition", message: "x" } },
  });
  render(<StaffDesk />);
  fireEvent.click(await screen.findByRole("button", { name: "Mark collected" }));
  fireEvent.click(screen.getByRole("button", { name: "Confirm collect" }));
  expect(await screen.findByRole("alert")).toHaveTextContent(/not in the right state.*refreshed/);
  expect(await screen.findByRole("button", { name: "Mark returned" })).toBeInTheDocument();
});

test("status filter and search narrow the list", async () => {
  fakeFetch({ "GET reservations": { status: 200, body: [
    row(), row({ reservationId: "r2", unitId: "ipad-u1", studentEmail: "s2@campus.test", status: "COLLECTED" }), row({ reservationId: "r3", unitId: "cam-u1", status: "RETURNED" }),
  ] } });
  render(<StaffDesk />);
  const table = await screen.findByRole("table");
  expect(within(table).getAllByRole("row")).toHaveLength(3); // header + 2 open
  fireEvent.change(screen.getByLabelText("Status"), { target: { value: "COLLECTED" } });
  expect(within(screen.getByRole("table")).getByText("ipad-u1")).toBeInTheDocument();
  expect(within(screen.getByRole("table")).queryByText("dell-u1")).toBeNull();
  fireEvent.change(screen.getByLabelText("Status"), { target: { value: "ALL" } });
  fireEvent.change(screen.getByLabelText("Find reservation"), { target: { value: "cam-u1" } });
  expect(within(screen.getByRole("table")).getAllByRole("row")).toHaveLength(2);
});

test("a 403 on the list is explicit", async () => {
  fakeFetch({ "GET reservations": { status: 403, body: { error: "ForbiddenRole", message: "x" } } });
  render(<StaffDesk />);
  expect(await screen.findByRole("alert")).toHaveTextContent("403 Forbidden.");
});
