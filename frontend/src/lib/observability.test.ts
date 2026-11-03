import { describe, expect, it } from "vitest";
import { correlationId, logProxy, routeTemplate } from "./observability";

describe("observability", () => {
  it("forwards safe correlation IDs and replaces unsafe ones", () => {
    expect(correlationId("abc-123")).toBe("abc-123");
    expect(correlationId(null)).toMatch(/^web-[0-9a-f-]{36}$/);
    expect(correlationId("bad\nvalue")).toMatch(/^web-/);
    expect(correlationId("x".repeat(200))).toMatch(/^web-/);
  });

  it("templates IDs out of routes", () => {
    expect(routeTemplate("reservations/0b6f3c1e-1111-4222-8333-944455556666/collect")).toBe("reservations/{id}/collect");
    expect(routeTemplate("catalogue")).toBe("catalogue");
  });

  it("writes one JSON line without tokens or emails", () => {
    const lines: string[] = [];
    logProxy({ method: "POST", route: "reservations", status: 409, durationMs: 12.4, correlationId: "c1" }, (l) => lines.push(l));
    const entry = JSON.parse(lines[0]);
    expect(entry).toMatchObject({ level: "warn", method: "POST", route: "reservations", status: 409, duration_ms: 12, correlation_id: "c1" });
    expect(Object.keys(entry).sort()).toEqual(
      ["correlation_id", "duration_ms", "event", "level", "method", "route", "service", "status", "timestamp"].sort()
    );
  });
});
