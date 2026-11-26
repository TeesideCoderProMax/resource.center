import { fireEvent, render, screen, within } from "@testing-library/react";
import { afterEach, beforeAll, expect, test, vi } from "vitest";
import type { StaffInventoryModel } from "@/lib/loan";
import { fakeFetch } from "@/test/fake-fetch";
import { StaffInventory } from "./StaffInventory";

const refreshMock = vi.fn();
vi.mock("next/navigation", () => ({
  useRouter: () => ({ refresh: refreshMock }),
}));

beforeAll(() => {
  if (typeof HTMLDialogElement !== "undefined") {
    HTMLDialogElement.prototype.showModal ??= function showModal() {
      this.setAttribute("open", "");
    };
    HTMLDialogElement.prototype.close ??= function close() {
      this.removeAttribute("open");
    };
  }
});

afterEach(() => {
  vi.unstubAllGlobals();
  refreshMock.mockReset();
});

const model = (o: Partial<StaffInventoryModel> = {}): StaffInventoryModel => ({
  id: "canon-r50",
  name: "Canon EOS R50",
  category: "Camera",
  description: "Mirrorless camera kit",
  loanDurationDays: 14,
  totalUnits: 2,
  availableUnits: 1,
  imageUrl: "/catalogue/canon-eos-r50.webp",
  imageAlt: "Canon EOS R50",
  specifications: [{ label: "Sensor", value: "24.2 MP APS-C" }],
  units: [
    { unitId: "cam-u1", serialNumber: "SN-CAM-001", status: "AVAILABLE" },
    { unitId: "cam-u2", serialNumber: "SN-CAM-002", status: "MAINTENANCE" },
  ],
  ...o,
});

test("renders live inventory, expands units, and toggles maintenance", async () => {
  const initial = [model()];
  const afterToggle = [
    model({
      availableUnits: 0,
      units: [
        { unitId: "cam-u1", serialNumber: "SN-CAM-001", status: "MAINTENANCE" },
        { unitId: "cam-u2", serialNumber: "SN-CAM-002", status: "MAINTENANCE" },
      ],
    }),
  ];
  const fetch = fakeFetch({
    "GET staff/inventory": [
      { status: 200, body: { items: initial } },
      { status: 200, body: { items: afterToggle } },
    ],
    "PATCH staff/inventory/units/cam-u1": {
      status: 200,
      body: { unitId: "cam-u1", serialNumber: "SN-CAM-001", status: "MAINTENANCE" },
    },
  });

  render(<StaffInventory />);
  const table = await screen.findByRole("table", { name: "Equipment inventory" });
  expect(within(table).getByText("Canon EOS R50")).toBeInTheDocument();
  expect(within(table).getByText("1 maintenance")).toBeInTheDocument();

  fireEvent.click(within(table).getByRole("button", { name: /Manage units \(2\)/ }));
  expect(await screen.findByText("SN-CAM-001")).toBeInTheDocument();

  const moveBtns = screen.getAllByRole("button", { name: "Move to maintenance" });
  fireEvent.click(moveBtns[0]);
  expect(await screen.findByRole("status")).toHaveTextContent(/Moved cam-u1 .* to maintenance/);
  expect(fetch).toHaveBeenCalledTimes(3);
  expect(refreshMock).toHaveBeenCalled();
});

test("adds and removes a unit and removes an unused model", async () => {
  const initial = [model()];
  const withAddedUnit = [
    model({
      totalUnits: 3,
      availableUnits: 2,
      units: [
        ...initial[0].units,
        { unitId: "cam-u3", serialNumber: "SN-CAM-003", status: "AVAILABLE" },
      ],
    }),
  ];

  fakeFetch({
    "GET staff/inventory": [
      { status: 200, body: { items: initial } },
      { status: 200, body: { items: withAddedUnit } },
      { status: 200, body: { items: initial } },
      { status: 200, body: { items: [] } },
    ],
    "POST staff/inventory/models/canon-r50/units": {
      status: 201,
      body: { unitId: "cam-u3", serialNumber: "SN-CAM-003", status: "AVAILABLE" },
    },
    "DELETE staff/inventory/units/cam-u3": { status: 200, body: { deleted: true } },
    "DELETE staff/inventory/models/canon-r50": { status: 200, body: { deleted: true } },
  });

  render(<StaffInventory />);
  const table = await screen.findByRole("table", { name: "Equipment inventory" });
  fireEvent.click(within(table).getByRole("button", { name: /Manage units \(2\)/ }));

  fireEvent.change(screen.getByLabelText(/New unit serial number/i), { target: { value: "SN-CAM-003" } });
  fireEvent.click(screen.getByRole("button", { name: "Add unit" }));
  expect(await screen.findByRole("status")).toHaveTextContent(/Added unit cam-u3/);
  expect(await screen.findByText("SN-CAM-003")).toBeInTheDocument();

  const row3 = screen.getByText("SN-CAM-003").closest("li")!;
  fireEvent.click(within(row3).getByRole("button", { name: "Remove" }));
  fireEvent.click(within(row3).getByRole("button", { name: "Confirm remove" }));
  expect(await screen.findByRole("status")).toHaveTextContent(/Removed unit cam-u3/);

  fireEvent.click(screen.getByRole("button", { name: "Remove model" }));
  fireEvent.click(screen.getByRole("button", { name: "Confirm remove" }));
  expect(await screen.findByRole("status")).toHaveTextContent(/Removed Canon EOS R50/);
});

test("add equipment modal requires all mandatory fields and submits valid payload", async () => {
  const created = model({ id: "sony-fx3", name: "Sony FX3", totalUnits: 1, availableUnits: 1 });
  const fetch = fakeFetch({
    "GET staff/inventory": [
      { status: 200, body: { items: [] } },
      { status: 200, body: { items: [created] } },
    ],
    "POST staff/inventory/models": { status: 201, body: created },
  });

  render(<StaffInventory />);
  await screen.findByText(/No equipment models match these filters/);

  fireEvent.click(screen.getByRole("button", { name: "Add equipment" }));
  const dialog = screen.getByRole("dialog", { name: "Add new equipment" });

  // Submitting incomplete form shows alert and does not POST
  fireEvent.click(within(dialog).getByRole("button", { name: "Save equipment" }));
  expect(within(dialog).getByRole("alert")).toHaveTextContent(/Complete all mandatory fields/);
  expect(fetch).toHaveBeenCalledTimes(1);

  fireEvent.change(within(dialog).getByLabelText(/Equipment name/i), { target: { value: "Sony FX3" } });
  fireEvent.change(within(dialog).getByLabelText(/Category/i), { target: { value: "Camera" } });
  fireEvent.change(within(dialog).getByLabelText(/Description/i), { target: { value: "Full-frame cinema camera" } });
  fireEvent.click(within(dialog).getByLabelText(/Image URL or \/catalogue\/ path/i));
  fireEvent.change(within(dialog).getByRole("textbox", { name: /^Image URL/i }), { target: { value: "/catalogue/canon-eos-r50.webp" } });
  fireEvent.change(within(dialog).getByLabelText(/Label 1/i), { target: { value: "Sensor" } });
  fireEvent.change(within(dialog).getByLabelText(/Value 1/i), { target: { value: "Full-frame" } });

  fireEvent.click(within(dialog).getByRole("button", { name: "Save equipment" }));
  expect(await screen.findByRole("status")).toHaveTextContent(/Added Sony FX3/);
  expect(fetch).toHaveBeenCalledTimes(3);
  const postCall = fetch.mock.calls[1];
  const sent = JSON.parse(String(postCall[1]?.body));
  expect(sent).toMatchObject({
    id: "sony-fx3",
    name: "Sony FX3",
    category: "Camera",
    description: "Full-frame cinema camera",
    loanDurationDays: 14,
    imageUrl: "/catalogue/canon-eos-r50.webp",
    imageAlt: "Sony FX3 loan equipment",
    specifications: [{ label: "Sensor", value: "Full-frame" }],
    units: [{ serialNumber: "CAMPUS-SONY-FX3-001", status: "AVAILABLE" }],
  });
});
