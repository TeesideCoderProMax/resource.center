import { fireEvent, render, screen } from "@testing-library/react";
import { expect, test } from "vitest";
import type { DeviceModelSummary } from "@/lib/loan";
import { Catalogue } from "./Catalogue";

const items: DeviceModelSummary[] = [
  { id: "ipad", name: "iPad Air", category: "tablet", description: "Tablet", loanDurationDays: 2, totalUnits: 2, availableUnits: 0 },
  { id: "dell", name: "Dell Latitude", category: "laptop", description: "Laptop", loanDurationDays: 2, totalUnits: 3, availableUnits: 2 },
];

const names = () => screen.getAllByRole("heading", { level: 3 }).map((h) => h.textContent);

test("shows totals from the catalogue and sorts by availability", () => {
  render(<Catalogue initialItems={items} initialError={null} canAct={false} />);
  expect(screen.getByText("2 of 2 models")).toBeInTheDocument();
  expect(names()).toEqual(["Dell Latitude", "iPad Air"]);
});

test("category chips, availability toggle and search filter the list", () => {
  render(<Catalogue initialItems={items} initialError={null} canAct={false} />);
  fireEvent.click(screen.getByRole("button", { name: /tablet/ }));
  expect(names()).toEqual(["iPad Air"]);
  fireEvent.click(screen.getByRole("checkbox", { name: "Available now only" }));
  expect(screen.getByText("No devices match these filters.")).toBeInTheDocument();
  fireEvent.click(screen.getByRole("button", { name: "Reset all" }));
  fireEvent.change(screen.getByRole("searchbox", { name: "Search" }), { target: { value: "dell" } });
  expect(names()).toEqual(["Dell Latitude"]);
});
