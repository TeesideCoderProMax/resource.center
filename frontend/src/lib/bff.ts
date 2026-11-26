/**
 * Pure rules for the /api/loan proxy (no Next.js imports, so `node --test` can run them).
 * Only the contract routes below are forwarded; everything else returns 404.
 */
const UUID = "[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}";
const MODEL_SLUG = "[a-z0-9][a-z0-9-]{0,62}[a-z0-9]";
const UNIT_ID = "[A-Za-z0-9._-]{1,128}";

type HttpMethod = "GET" | "POST" | "PATCH" | "DELETE";

const ROUTES: { method: HttpMethod; pattern: RegExp; auth: boolean }[] = [
  { method: "GET", pattern: /^catalogue$/, auth: false },
  { method: "GET", pattern: /^reservations$/, auth: true },
  { method: "POST", pattern: /^reservations$/, auth: true },
  { method: "POST", pattern: /^subscriptions$/, auth: true },
  { method: "POST", pattern: new RegExp(`^reservations/${UUID}/(collect|return)$`), auth: true },
  { method: "GET", pattern: /^staff\/inventory$/, auth: true },
  { method: "POST", pattern: /^staff\/inventory\/models$/, auth: true },
  { method: "DELETE", pattern: new RegExp(`^staff/inventory/models/${MODEL_SLUG}$`), auth: true },
  { method: "POST", pattern: new RegExp(`^staff/inventory/models/${MODEL_SLUG}/units$`), auth: true },
  { method: "PATCH", pattern: new RegExp(`^staff/inventory/units/${UNIT_ID}$`), auth: true },
  { method: "DELETE", pattern: new RegExp(`^staff/inventory/units/${UNIT_ID}$`), auth: true },
];

export function matchRoute(method: string, path: string): { auth: boolean } | null {
  const route = ROUTES.find((r) => r.method === method && r.pattern.test(path));
  return route ? { auth: route.auth } : null;
}

/**
 * CSRF guard for state-changing requests: the session cookie is SameSite=Lax, and mutating calls must
 * also carry an Origin header equal to the app origin. Missing Origin on mutating calls is rejected.
 */
export function sameOrigin(method: string, origin: string | null, appBaseUrl: string): boolean {
  if (method === "GET") return true;
  if (!origin || !appBaseUrl) return false;
  try {
    return new URL(origin).origin === new URL(appBaseUrl).origin;
  } catch {
    return false;
  }
}

const asObj = (v: unknown): Record<string, unknown> => (typeof v === "object" && v !== null ? (v as Record<string, unknown>) : {});
const asStr = (v: unknown): string => (typeof v === "string" ? v : "");

/** Forward only contract fields; identity always comes from the verified token. */
export function sanitizeBody(path: string, body: unknown): string | undefined {
  const o = asObj(body);
  if (path === "reservations" || path === "subscriptions") {
    const modelId = asStr(o.modelId);
    const loanDurationDays = path === "reservations" ? o.loanDurationDays : undefined;
    return JSON.stringify({ modelId, ...(path === "reservations" ? { loanDurationDays } : {}) });
  }
  if (path === "staff/inventory/models") {
    const specifications = Array.isArray(o.specifications)
      ? o.specifications.map((s) => { const item = asObj(s); return { label: asStr(item.label), value: asStr(item.value) }; })
      : [];
    const units = Array.isArray(o.units)
      ? o.units.map((u) => { const item = asObj(u); return { serialNumber: asStr(item.serialNumber), status: asStr(item.status) }; })
      : [];
    return JSON.stringify({
      id: asStr(o.id),
      name: asStr(o.name),
      category: asStr(o.category),
      description: asStr(o.description),
      loanDurationDays: o.loanDurationDays,
      imageUrl: asStr(o.imageUrl),
      imageAlt: asStr(o.imageAlt),
      specifications,
      units,
    });
  }
  if (new RegExp(`^staff/inventory/models/${MODEL_SLUG}/units$`).test(path)) {
    return JSON.stringify({ serialNumber: asStr(o.serialNumber), status: asStr(o.status) });
  }
  if (new RegExp(`^staff/inventory/units/${UNIT_ID}$`).test(path)) {
    return JSON.stringify({ status: asStr(o.status) });
  }
  return undefined;
}
