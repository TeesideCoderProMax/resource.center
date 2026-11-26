import { test } from "vitest";
import assert from "node:assert/strict";
import { matchRoute, sameOrigin, sanitizeBody } from "./bff.ts";

const id = "0f8fad5b-d9cb-469f-a165-70867728950e";

test("only contract routes are forwarded", () => {
  assert.deepEqual(matchRoute("GET", "catalogue"), { auth: false });
  assert.deepEqual(matchRoute("GET", "reservations"), { auth: true });
  assert.deepEqual(matchRoute("POST", `reservations/${id}/collect`), { auth: true });
  assert.deepEqual(matchRoute("POST", `reservations/${id}/return`), { auth: true });
  assert.deepEqual(matchRoute("GET", "staff/inventory"), { auth: true });
  assert.deepEqual(matchRoute("POST", "staff/inventory/models"), { auth: true });
  assert.deepEqual(matchRoute("DELETE", "staff/inventory/models/sony-fx3"), { auth: true });
  assert.deepEqual(matchRoute("POST", "staff/inventory/models/sony-fx3/units"), { auth: true });
  assert.deepEqual(matchRoute("PATCH", "staff/inventory/units/unit-1"), { auth: true });
  assert.deepEqual(matchRoute("DELETE", "staff/inventory/units/unit-1"), { auth: true });
  assert.equal(matchRoute("POST", "reservations/1/collect"), null);
  assert.equal(matchRoute("POST", "catalogue"), null);
  assert.equal(matchRoute("GET", "../health/ready"), null);
  assert.equal(matchRoute("DELETE", "reservations"), null);
  assert.equal(matchRoute("DELETE", "staff/inventory/models/Bad Slug"), null);
});

test("cross-site mutating calls are rejected", () => {
  const app = "https://rc.example.edu";
  assert.equal(sameOrigin("GET", null, app), true);
  assert.equal(sameOrigin("POST", "https://rc.example.edu", app), true);
  assert.equal(sameOrigin("PATCH", "https://rc.example.edu", app), true);
  assert.equal(sameOrigin("DELETE", "https://evil.example", app), false);
  assert.equal(sameOrigin("POST", "https://evil.example", app), false);
  assert.equal(sameOrigin("POST", null, app), false);
  assert.equal(sameOrigin("POST", "https://rc.example.edu", ""), false);
});

test("client cannot inject identity, email or role", () => {
  const body = sanitizeBody("reservations", { modelId: "m1", studentEmail: "x@y", roles: ["staff"], sub: "a" });
  assert.equal(body, JSON.stringify({ modelId: "m1" }));
  assert.equal(sanitizeBody(`reservations/${id}/collect`, { studentEmail: "x@y" }), undefined);
});

test("forwards the chosen reservation period without forwarding identity", () => {
  assert.deepEqual(JSON.parse(sanitizeBody("reservations", { modelId: "m1", loanDurationDays: 14, studentEmail: "x@y", roles: ["staff"] })!), { modelId: "m1", loanDurationDays: 14 });
  assert.equal(sanitizeBody("subscriptions", { modelId: "m1", loanDurationDays: 14 }), JSON.stringify({ modelId: "m1" }));
});

test("sanitizes staff inventory payloads", () => {
  const modelBody = JSON.parse(sanitizeBody("staff/inventory/models", {
    id: "sony-fx3", name: "Sony FX3", category: "Camera", description: "Cinema", loanDurationDays: 7,
    imageUrl: "/catalogue/canon-eos-r50.webp", imageAlt: "Sony FX3",
    specifications: [{ label: "Sensor", value: "Full-frame", extra: "drop" }],
    units: [{ serialNumber: "SN-1", status: "AVAILABLE", owner: "evil" }],
    roles: ["staff"],
  })!);
  assert.deepEqual(Object.keys(modelBody).sort(), [
    "category", "description", "id", "imageAlt", "imageUrl", "loanDurationDays", "name", "specifications", "units",
  ]);
  assert.deepEqual(modelBody.specifications, [{ label: "Sensor", value: "Full-frame" }]);
  assert.deepEqual(modelBody.units, [{ serialNumber: "SN-1", status: "AVAILABLE" }]);
  assert.equal(sanitizeBody("staff/inventory/units/u1", { status: "MAINTENANCE", roles: ["staff"] }), JSON.stringify({ status: "MAINTENANCE" }));
  assert.equal(sanitizeBody("staff/inventory/models/sony-fx3/units", { serialNumber: "SN-2", status: "AVAILABLE", extra: true }), JSON.stringify({ serialNumber: "SN-2", status: "AVAILABLE" }));
});
