/**
 * Display-only role helpers shared by server pages and tests. The Loan API verifies the
 * token and enforces RBAC; these only decide what the UI shows.
 */
export interface Viewer {
  authConfigured: boolean;
  signedIn: boolean;
  email: string | null;
  roles: string[];
}

export type Access = "ok" | "auth-not-configured" | "sign-in" | "no-role" | "forbidden";

export const isStaff = (v: Viewer) => v.roles.includes("staff");
export const canBorrow = (v: Viewer) => isStaff(v) || v.roles.includes("student");

export function access(v: Viewer, need: "member" | "staff"): Access {
  if (!v.authConfigured) return "auth-not-configured";
  if (!v.signedIn) return "sign-in";
  if (!canBorrow(v)) return "no-role";
  if (need === "staff" && !isStaff(v)) return "forbidden";
  return "ok";
}

export function roleLabel(v: Viewer): string {
  if (isStaff(v)) return "staff";
  if (v.roles.includes("student")) return "student";
  return "no role";
}

export function navLinks(v: Viewer): { href: string; label: string }[] {
  const links = [{ href: "/", label: "Catalog" }];
  if (v.signedIn && canBorrow(v)) links.push({ href: "/reservations", label: isStaff(v) ? "All Loans" : "My Loans" });
  if (isStaff(v)) links.push({ href: "/staff", label: "Staff Admin" });
  return links;
}
