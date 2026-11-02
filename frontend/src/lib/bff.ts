/**
 * Pure rules for the /api/loan proxy (no Next.js imports, so `node --test` can run them).
 * Only the contract routes below are forwarded; everything else returns 404.
 */
const UUID = "[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}";

const ROUTES: { method: "GET" | "POST"; pattern: RegExp; auth: boolean }[] = [
  { method: "GET", pattern: /^catalogue$/, auth: false },
  { method: "GET", pattern: /^reservations$/, auth: true },
  { method: "POST", pattern: /^reservations$/, auth: true },
  { method: "POST", pattern: /^subscriptions$/, auth: true },
  { method: "POST", pattern: new RegExp(`^reservations/${UUID}/(collect|return)$`), auth: true },
];

export function matchRoute(method: string, path: string): { auth: boolean } | null {
  const route = ROUTES.find((r) => r.method === method && r.pattern.test(path));
  return route ? { auth: route.auth } : null;
}

/**
 * CSRF guard for state-changing requests: the session cookie is SameSite=Lax, and POSTs must
 * also carry an Origin header equal to the app origin. Missing Origin on POST is rejected.
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

/** Only `modelId` is forwarded. Actor identity, email and role come from the token. */
export function sanitizeBody(path: string, body: unknown): string | undefined {
  if (path === "reservations" || path === "subscriptions") {
    const modelId = typeof body === "object" && body !== null ? (body as Record<string, unknown>).modelId : undefined;
    return JSON.stringify({ modelId: typeof modelId === "string" ? modelId : "" });
  }
  return undefined;
}
