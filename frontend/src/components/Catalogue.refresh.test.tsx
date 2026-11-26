import { act, render, screen } from "@testing-library/react";
import { expect, test, vi } from "vitest";
import type { DeviceModelSummary } from "@/lib/loan";
import { Catalogue } from "./Catalogue";

let refresh: (() => Promise<void>) | undefined;
vi.mock("./DeviceCard", () => ({
  DeviceCard: ({ item, onChanged }: { item: DeviceModelSummary; onChanged: () => Promise<void> }) => { refresh = onChanged; return <h3>{item.name}</h3>; },
}));
const many: DeviceModelSummary[] = Array.from({ length: 9 }, (_, i) => ({ id: `d${i}`, name: `Dell ${i}`, category: "laptop", description: "", loanDurationDays: 7, totalUnits: 2, availableUnits: 2 }));
vi.mock("@/lib/api-client", () => ({ call: vi.fn(async () => ({ items: many.map((m) => (m.id === "d0" ? { ...m, availableUnits: 0 } : m)) })) }));

test("a refresh after reserving keeps the card on its page and in place", async () => {
  render(<Catalogue initialItems={many} initialError={null} canAct={false} />);
  const names = () => screen.getAllByRole("heading", { level: 3 }).map((h) => h.textContent);
  expect(names()[0]).toBe("Dell 0");
  await act(async () => { await refresh!(); });
  expect(names()[0]).toBe("Dell 0");
});
