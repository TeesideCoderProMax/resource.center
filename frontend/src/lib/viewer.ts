import { auth0, auth0Configured } from "./auth0";
import { displayRoles, loanApiBaseUrl, type DeviceModelSummary } from "./loan";
import type { Viewer } from "./roles";

export async function getViewer(): Promise<Viewer> {
  const session = auth0 ? await auth0.getSession() : null;
  return {
    authConfigured: auth0Configured,
    signedIn: Boolean(session),
    email: session ? (session.user.email ?? session.user.name ?? null) : null,
    roles: displayRoles(session?.tokenSet.accessToken),
  };
}

export async function fetchCatalogue(): Promise<{ items: DeviceModelSummary[]; error: string | null }> {
  const base = loanApiBaseUrl();
  if (!base) return { items: [], error: "LOAN_API_BASE_URL is not configured" };
  try {
    const res = await fetch(`${base}/api/v1/catalogue`, { cache: "no-store" });
    if (!res.ok) return { items: [], error: `Loan API returned HTTP ${res.status}` };
    return { items: ((await res.json()) as { items: DeviceModelSummary[] }).items, error: null };
  } catch {
    return { items: [], error: "Could not reach the Loan API" };
  }
}
