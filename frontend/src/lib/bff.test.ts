import { test } from "node:test";
import assert from "node:assert/strict";
import { matchRoute, sameOrigin, sanitizeBody } from "./bff.ts";

const id = "0f8fad5b-d9cb-469f-a165-70867728950e";

test("only contract routes are forwarded", () => {
  assert.deepEqual(matchRoute("GET", "catalogue"), { auth: false });
  assert.deepEqual(matchRoute("GET", "reservations"), { auth: true });
  assert.deepEqual(matchRoute("POST", `reservations/${id}/collect`), { auth: true });
  assert.deepEqual(matchRoute("POST", `reservations/${id}/return`), { auth: true });
  assert.equal(matchRoute("POST", "reservations/1/collect"), null);
  assert.equal(matchRoute("POST", "catalogue"), null);
  assert.equal(matchRoute("GET", "../health/ready"), null);
  assert.equal(matchRoute("DELETE", "reservations"), null);
});

test("cross-site POSTs are rejected", () => {
  const app = "https://rc.example.edu";
  assert.equal(sameOrigin("GET", null, app), true);
  assert.equal(sameOrigin("POST", "https://rc.example.edu", app), true);
  assert.equal(sameOrigin("POST", "https://evil.example", app), false);
  assert.equal(sameOrigin("POST", null, app), false);
  assert.equal(sameOrigin("POST", "https://rc.example.edu", ""), false);
});

test("client cannot inject identity, email or role", () => {
  const body = sanitizeBody("reservations", { modelId: "m1", studentEmail: "x@y", roles: ["staff"], sub: "a" });
  assert.equal(body, JSON.stringify({ modelId: "m1" }));
  assert.equal(sanitizeBody(`reservations/${id}/collect`, { studentEmail: "x@y" }), undefined);
});
