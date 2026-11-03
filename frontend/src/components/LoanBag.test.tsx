import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, expect, test, vi } from "vitest";
import { BagProvider } from "@/lib/bag";
import type { DeviceModelSummary } from "@/lib/loan";
import { fakeFetch } from "@/test/fake-fetch";
import { LoanBag } from "./LoanBag";

vi.mock("next/navigation", () => ({ useRouter: () => ({ refresh: vi.fn(), push: vi.fn() }) }));

const catalogue: DeviceModelSummary[] = [
  { id: "dell", name: "Dell Latitude", category: "laptop", description: "d", loanDurationDays: 2, totalUnits: 2, availableUnits: 2 },
  { id: "ipad", name: "iPad Air", category: "tablet", description: "d", loanDurationDays: 2, totalUnits: 1, availableUnits: 1 },
];

// The jsdom used here has no localStorage, so the bag reads from an in-memory stub.
beforeEach(() => {
  const store = new Map([["loan-bag:v1", JSON.stringify(["dell", "ipad", "gone"])]]);
  vi.stubGlobal("localStorage", { getItem: (k: string) => store.get(k) ?? null, setItem: (k: string, v: string) => store.set(k, v) });
});
afterEach(() => vi.unstubAllGlobals());

const renderBag = () => render(<BagProvider><LoanBag catalogue={catalogue} catalogueError={null} email="s1@campus.test" role="student" /></BagProvider>);
const acknowledge = () => screen.getAllByRole("checkbox").forEach((c) => fireEvent.click(c));

test("lists bag items, flags stale ones and gates submit on the loan terms", async () => {
  renderBag();
  expect(await screen.findByRole("heading", { name: "Dell Latitude" })).toBeInTheDocument();
  expect(screen.getByText(/1 item is no longer in the catalogue/)).toBeInTheDocument();
  const submit = screen.getByRole("button", { name: "Reserve all (2)" });
  expect(submit).toBeDisabled();
  acknowledge();
  expect(submit).toBeEnabled();
  fireEvent.click(screen.getAllByRole("button", { name: "Remove item" })[1]);
  expect(screen.getByRole("button", { name: "Reserve all (1)" })).toBeInTheDocument();
});

test("reserves each item separately and reports a mixed outcome", async () => {
  const fetch = fakeFetch({
    "POST reservations": [
      { status: 201, body: { unitId: "dell-u1", dueAt: "2099-01-03T00:00:00Z" } },
      { status: 409, body: { error: "NoAvailableUnits", message: "x" } },
    ],
    "POST subscriptions": { status: 201, body: { subscriptionId: "s1" } },
  });
  renderBag();
  await screen.findByRole("heading", { name: "Dell Latitude" });
  acknowledge();
  const submit = screen.getByRole("button", { name: "Reserve all (2)" });
  fireEvent.click(submit);
  fireEvent.click(submit);
  expect(await screen.findByText(/Reserved Dell Latitude \(unit dell-u1\)/)).toBeInTheDocument();
  expect(await screen.findByRole("alert")).toHaveTextContent("No units left");
  expect(fetch).toHaveBeenCalledTimes(2);
  // The reserved item leaves the bag; the failed one stays with a waitlist option.
  expect(screen.queryByRole("heading", { name: "Dell Latitude" })).toBeNull();
  fireEvent.click(screen.getByRole("button", { name: "Notify me when available instead" }));
  await waitFor(() => expect(screen.getByText(/emailed when iPad Air is available/)).toBeInTheDocument());
});
