import { expect, test } from "vitest";
import { access, navLinks, roleLabel, type Viewer } from "./roles";

const v = (o: Partial<Viewer>): Viewer => ({ authConfigured: true, signedIn: true, email: "a@b", roles: [], ...o });

test("guards report the right blocked state", () => {
  expect(access(v({ authConfigured: false }), "member")).toBe("auth-not-configured");
  expect(access(v({ signedIn: false }), "member")).toBe("sign-in");
  expect(access(v({ roles: [] }), "member")).toBe("no-role");
  expect(access(v({ roles: ["student"] }), "staff")).toBe("forbidden");
  expect(access(v({ roles: ["student"] }), "member")).toBe("ok");
  expect(access(v({ roles: ["staff"] }), "staff")).toBe("ok");
});

test("navigation and badge follow the display role", () => {
  expect(navLinks(v({ signedIn: false })).map((l) => l.href)).toEqual(["/"]);
  expect(navLinks(v({ roles: ["student"] })).map((l) => l.href)).toEqual(["/", "/reservations"]);
  expect(navLinks(v({ roles: ["staff"] })).map((l) => l.href)).toEqual(["/", "/reservations", "/staff"]);
  expect(roleLabel(v({ roles: ["staff", "student"] }))).toBe("staff");
  expect(roleLabel(v({}))).toBe("no role");
});
