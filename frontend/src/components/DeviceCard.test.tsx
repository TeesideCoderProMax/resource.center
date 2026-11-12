import { fireEvent, render, screen } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";
import type { DeviceModelSummary } from "@/lib/loan";
import { fakeFetch } from "@/test/fake-fetch";
import { DeviceCard } from "./DeviceCard";

afterEach(() => vi.unstubAllGlobals());

const item: DeviceModelSummary = { id: "ipad", name: "iPad", category: "tablet", description: "d", loanDurationDays: 2, totalUnits: 1, availableUnits: 1 };

test("reserve shows the due date, next steps and reloads from the API", async () => {
  fakeFetch({ "POST reservations": { status: 201, body: { unitId: "ipad-u1", dueAt: "2026-11-04T10:00:00Z" } } });
  const onChanged = vi.fn();
  render(<DeviceCard item={item} canAct onChanged={onChanged} />);
  fireEvent.click(screen.getByRole("button", { name: "Reserve" }));
  expect(screen.getByRole("button", { name: "Reserving…" })).toBeDisabled();
  expect(await screen.findByRole("status")).toHaveTextContent(/Reserved iPad \(unit ipad-u1\)\. Due back .*collect it at the Resource Center desk/);
  expect(screen.getByRole("link", { name: "View my reservations" })).toHaveAttribute("href", "/reservations");
  expect(onChanged).toHaveBeenCalledOnce();
  // No optimistic count change: the badge only changes when the parent reloads.
  expect(screen.getByText("1 / 1 available")).toBeInTheDocument();
});

test("double clicks send one request", async () => {
  const fetch = fakeFetch({ "POST reservations": { status: 201, body: { unitId: "u", dueAt: "2026-11-04T10:00:00Z" } } });
  render(<DeviceCard item={item} canAct />);
  const btn = screen.getByRole("button", { name: "Reserve" });
  fireEvent.click(btn);
  fireEvent.click(btn);
  await screen.findByRole("status");
  expect(fetch).toHaveBeenCalledOnce();
});

test("409 last-unit race offers subscribe, which confirms an email", async () => {
  fakeFetch({
    "POST reservations": { status: 409, body: { error: "NoAvailableUnits", message: "x" } },
    "POST subscriptions": { status: 201, body: { subscriptionId: "s1" } },
  });
  render(<DeviceCard item={item} canAct />);
  fireEvent.click(screen.getByRole("button", { name: "Reserve" }));
  expect(await screen.findByRole("alert")).toHaveTextContent("No units left. Someone took the last one. You can subscribe instead. (ref web-test)");
  fireEvent.click(screen.getByRole("button", { name: "Notify me when available" }));
  expect(await screen.findByRole("status")).toHaveTextContent("You will be emailed when iPad is available.");
});

test("401 offers re-sign-in back to the device page", async () => {
  fakeFetch({ "POST reservations": { status: 401, body: { error: "Unauthorized", message: "x" } } });
  render(<DeviceCard item={item} canAct />);
  fireEvent.click(screen.getByRole("button", { name: "Reserve" }));
  expect(await screen.findByRole("link", { name: "Sign in again" })).toHaveAttribute("href", "/auth/login?returnTo=%2Fdevices%2Fipad");
});

test("403 ForbiddenRole is explicit", async () => {
  fakeFetch({ "POST reservations": { status: 403, body: { error: "ForbiddenRole", message: "x" } } });
  render(<DeviceCard item={item} canAct />);
  fireEvent.click(screen.getByRole("button", { name: "Reserve" }));
  expect(await screen.findByRole("alert")).toHaveTextContent("403 Forbidden. Your role does not allow this action. (ref web-test)");
});

test("signed-out viewers see no action buttons", () => {
  render(<DeviceCard item={item} canAct={false} />);
  expect(screen.queryByRole("button")).toBeNull();
});

test("sends the chosen whole-day period with the reservation", async () => {
  const fetch = fakeFetch({ "POST reservations": { status: 201, body: { unitId: "u", dueAt: "2026-11-18T10:00:00Z" } } });
  render(<DeviceCard item={{ ...item, loanDurationDays: 14 }} canAct />);
  const selector = screen.getByRole("combobox", { name: "Loan period for iPad" });
  expect(selector).toHaveValue("1");
  fireEvent.change(selector, { target: { value: "14" } });
  fireEvent.click(screen.getByRole("button", { name: "Reserve" }));
  await screen.findByRole("status");
  expect(JSON.parse(String(fetch.mock.calls[0][1]?.body))).toEqual({ modelId: "ipad", loanDurationDays: 14 });
});

test("shows a product image and falls back when it fails", () => {
  render(<DeviceCard item={{ ...item, imageUrl: "/catalogue/ipad.webp", imageAlt: "iPad with Pencil" }} canAct={false} />);
  const img = screen.getByRole("img", { name: "iPad with Pencil" });
  expect(img).toHaveAttribute("src", "/catalogue/ipad.webp");
  fireEvent.error(img);
  expect(screen.queryByRole("img")).toBeNull();
  expect(screen.getByText("I")).toBeInTheDocument();
});
