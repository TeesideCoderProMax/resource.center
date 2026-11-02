import { afterEach, expect, test, vi } from "vitest";
import { ApiClientError, call, describe, errorMessage } from "./api-client";

afterEach(() => vi.unstubAllGlobals());

test("maps known API error codes to friendly text", () => {
  expect(errorMessage(409, { error: "NoAvailableUnits", message: "x" })).toMatch(/No units left/);
  expect(errorMessage(403, { error: "ForbiddenRole", message: "x" })).toMatch(/role does not allow/);
});

test("falls back to the API message, then the HTTP status", () => {
  expect(errorMessage(400, { error: "Unknown", message: "bad thing" })).toBe("bad thing");
  expect(errorMessage(500, null)).toBe("Request failed (HTTP 500)");
});

test("call() throws ApiClientError with the response correlation ID", async () => {
  vi.stubGlobal("fetch", vi.fn(async () => new Response(JSON.stringify({ error: "InvalidTransition", message: "no" }), {
    status: 409, headers: { "x-correlation-id": "web-abc" },
  })));
  const err = (await call("POST", "reservations/x/collect").catch((e) => e)) as ApiClientError;
  expect(err).toBeInstanceOf(ApiClientError);
  expect(err.status).toBe(409);
  expect(err.correlationId).toBe("web-abc");
  expect(describe(err)).toBe("That reservation is not in the right state for this action. (ref web-abc)");
});

test("call() only talks to the same-origin proxy", async () => {
  const fetch = vi.fn(async (_url: string) => new Response("{\"items\":[]}", { status: 200 }));
  vi.stubGlobal("fetch", fetch);
  await call("GET", "catalogue");
  expect(fetch.mock.calls[0][0]).toBe("/api/loan/catalogue");
});
