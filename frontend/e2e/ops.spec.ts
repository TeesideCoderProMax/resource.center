import { expect, test } from "@playwright/test";

test("liveness and readiness report config and Loan API reachability", async ({ request }) => {
  const live = await request.get("/api/health");
  expect(live.status()).toBe(200);
  const ready = await request.get("/api/ready");
  expect(ready.status()).toBe(200);
  expect(await ready.json()).toMatchObject({ status: "ready", loanApi: "reachable", auth0Configured: true });
});

test("proxy forwards a safe correlation ID and replaces an unsafe one", async ({ request }) => {
  const kept = await request.get("/api/loan/catalogue", { headers: { "x-correlation-id": "demo-trace-1" } });
  expect(kept.headers()["x-correlation-id"]).toBe("demo-trace-1");
  const replaced = await request.get("/api/loan/catalogue", { headers: { "x-correlation-id": "bad value!" } });
  expect(replaced.headers()["x-correlation-id"]).toMatch(/^web-/);
  const missing = await request.get("/api/loan/unknown");
  expect(missing.status()).toBe(404);
  expect(missing.headers()["x-correlation-id"]).toMatch(/^web-/);
});
