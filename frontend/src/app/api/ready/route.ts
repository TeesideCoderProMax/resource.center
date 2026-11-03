import { NextResponse } from "next/server";
import { auth0Configured } from "@/lib/auth0";
import { loanApiBaseUrl } from "@/lib/loan";

export const dynamic = "force-dynamic";

/**
 * Readiness: required config is present and the Loan API answers its liveness route.
 * Auth0 may be absent only when ALLOW_UNCONFIGURED_AUTH=true (approved local/fake smoke).
 */
export async function GET() {
  const base = loanApiBaseUrl();
  const authOk = auth0Configured || process.env.ALLOW_UNCONFIGURED_AUTH === "true";
  let loanApi: "reachable" | "unreachable" | "not_configured" = "not_configured";
  if (base) {
    try {
      const res = await fetch(`${base}/health/live`, { cache: "no-store", signal: AbortSignal.timeout(2500) });
      loanApi = res.ok ? "reachable" : "unreachable";
    } catch {
      loanApi = "unreachable";
    }
  }
  const ready = authOk && Boolean(process.env.APP_BASE_URL) && loanApi === "reachable";
  return NextResponse.json(
    {
      status: ready ? "ready" : "not_ready",
      service: "resource-center-web",
      version: process.env.RC_VERSION ?? "dev",
      auth0Configured,
      appBaseUrlConfigured: Boolean(process.env.APP_BASE_URL),
      loanApi,
    },
    { status: ready ? 200 : 503, headers: { "cache-control": "no-store" } }
  );
}
