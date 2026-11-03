import { expect, request, test } from "@playwright/test";
import { API_URL, BASE_URL } from "./env";
import { signIn } from "./session";

test.beforeEach(async () => {
  const api = await request.newContext();
  await api.post(`${API_URL}/__reset`);
  await api.dispose();
});

async function seed(body: object) {
  const api = await request.newContext();
  const res = await api.post(`${API_URL}/__seed`, { data: body });
  const row = await res.json();
  await api.dispose();
  return row as { reservationId: string; unitId: string };
}

test("public catalogue renders signed out", async ({ page }) => {
  await page.goto("/");
  await expect(page.getByRole("heading", { name: "Devices" })).toBeVisible();
  await expect(page.getByText("Dell Latitude 7440")).toBeVisible();
  await expect(page.getByRole("button", { name: "Reserve" })).toHaveCount(0);
});

test("student reserves and sees due date and next steps", async ({ page, context }) => {
  await signIn(context, "student");
  await page.goto("/devices/dell-latitude-7440");
  await expect(page.getByText("2 / 2 available")).toBeVisible();
  await page.getByRole("button", { name: "Reserve" }).click();
  await expect(page.getByRole("status")).toContainText(/Due back .*collect it at the Resource Center desk/);
  await expect(page.getByText("1 / 2 available")).toBeVisible();
  await page.getByRole("link", { name: "View my reservations" }).click();
  await expect(page.getByRole("heading", { name: "My reservations" })).toBeVisible();
  await expect(page.getByRole("cell", { name: "RESERVED" })).toBeVisible();
});

test("last-unit 409 suggests subscribe, then subscribe confirms", async ({ browser }) => {
  const a = await browser.newContext();
  const b = await browser.newContext();
  await signIn(a, "student");
  await signIn(b, "student2");
  const pa = await a.newPage();
  const pb = await b.newPage();
  await pa.goto(`${BASE_URL}/devices/ipad-air-m2`);
  await pb.goto(`${BASE_URL}/devices/ipad-air-m2`);
  await pa.getByRole("button", { name: "Reserve" }).click();
  await expect(pa.getByRole("status")).toContainText("Reserved iPad Air M2");

  await pb.getByRole("button", { name: "Reserve" }).click();
  await expect(pb.getByRole("main").getByRole("alert")).toContainText(/No units left.*subscribe.*\(ref /);
  await pb.getByRole("button", { name: "Notify me when available" }).click();
  await expect(pb.getByRole("status")).toContainText("You will be emailed when iPad Air M2 is available");
  await a.close();
  await b.close();
});

test("staff collects and returns with confirm, timestamps and refresh", async ({ page, context }) => {
  await seed({ modelId: "dell-latitude-7440", sub: "auth0|student-1", email: "student1@campus.test" });
  await signIn(context, "staff");
  await page.goto("/staff");
  await page.getByLabel("Status").selectOption("RESERVED");
  await page.getByRole("button", { name: "Mark collected" }).click();
  await page.getByRole("button", { name: "Cancel" }).click();
  await page.getByRole("button", { name: "Mark collected" }).click();
  await page.getByRole("button", { name: "Confirm collect" }).click();
  await expect(page.getByRole("status")).toContainText(/Collected dell-latitude-7440-u1 at .* Confirmation email queued/);
  await expect(page.getByText("No reservations.")).toBeVisible();

  await page.getByLabel("Status").selectOption("COLLECTED");
  await page.getByLabel("Find reservation").fill("student1");
  await page.getByRole("button", { name: "Mark returned" }).click();
  await page.getByRole("button", { name: "Confirm return" }).click();
  await expect(page.getByRole("status")).toContainText(/Returned dell-latitude-7440-u1 at .*subscribers will be notified/);
  await page.getByLabel("Status").selectOption("RETURNED");
  await expect(page.getByRole("cell", { name: "RETURNED" })).toBeVisible();
});

test("overdue loans are highlighted", async ({ page, context }) => {
  await seed({ modelId: "dell-latitude-7440", sub: "auth0|student-1", email: "student1@campus.test", status: "COLLECTED", reservedHoursAgo: 72 });
  await signIn(context, "student");
  await page.goto("/reservations");
  await expect(page.getByText("Overdue")).toBeVisible();
});

test("student gets 403 on /staff and on a forced collect call", async ({ page, context }) => {
  const row = await seed({ modelId: "dell-latitude-7440", sub: "auth0|student-1", email: "student1@campus.test" });
  await signIn(context, "student");
  await page.goto("/staff");
  await expect(page.getByRole("heading", { name: "403 — staff only" })).toBeVisible();
  await expect(page.getByRole("link", { name: "Staff desk" })).toHaveCount(0);

  const res = await page.request.post(`/api/loan/reservations/${row.reservationId}/collect`, { headers: { origin: BASE_URL } });
  expect(res.status()).toBe(403);
  expect((await res.json()).error).toBe("ForbiddenRole");
  expect(res.headers()["x-correlation-id"]).toBeTruthy();
});

test("cross-origin POST is rejected by the proxy", async ({ page, context }) => {
  await signIn(context, "staff");
  await page.goto("/");
  const res = await page.request.post("/api/loan/reservations", { headers: { origin: "https://evil.example" }, data: { modelId: "dell-latitude-7440" } });
  expect(res.status()).toBe(403);
  expect((await res.json()).error).toBe("CrossOriginRejected");
});

test("signed-out proxy call returns 401 and pages prompt sign-in", async ({ page }) => {
  const res = await page.request.post("/api/loan/reservations", { headers: { origin: BASE_URL }, data: { modelId: "dell-latitude-7440" } });
  expect(res.status()).toBe(401);
  await page.goto("/reservations");
  await expect(page.getByRole("link", { name: "Sign in" }).last()).toHaveAttribute("href", "/auth/login?returnTo=%2Freservations");
});

test("identity in the request body is ignored; the token decides", async ({ page, context }) => {
  await signIn(context, "student");
  await page.goto("/");
  const res = await page.request.post("/api/loan/reservations", {
    headers: { origin: BASE_URL },
    data: { modelId: "ipad-air-m2", studentEmail: "attacker@evil.example", roles: ["staff"] },
  });
  expect(res.status()).toBe(201);
  const rows = await (await page.request.get("/api/loan/reservations")).json();
  expect(rows[0].studentEmail).toBe("student1@campus.test");
});
