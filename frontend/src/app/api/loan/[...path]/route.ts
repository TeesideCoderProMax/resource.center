import { NextRequest, NextResponse } from "next/server";
import { auth0 } from "@/lib/auth0";
import { matchRoute, sameOrigin, sanitizeBody } from "@/lib/bff";
import { loanApiBaseUrl } from "@/lib/loan";
import { correlationId, logProxy, routeTemplate } from "@/lib/observability";

export const dynamic = "force-dynamic";

function error(status: number, code: string, message: string, correlation: string) {
  return NextResponse.json(
    { error: code, message, correlationId: correlation },
    { status, headers: { "x-correlation-id": correlation } }
  );
}

/** Wraps the proxy with one structured log line per request (no tokens, emails or bodies). */
async function handle(req: NextRequest, ctx: { params: Promise<{ path: string[] }> }) {
  const started = performance.now();
  const correlation = correlationId(req.headers.get("x-correlation-id"));
  const path = (await ctx.params).path.join("/");
  const res = await forward(req, path, correlation);
  logProxy({
    method: req.method,
    route: matchRoute(req.method, path) ? routeTemplate(path) : "unmatched",
    status: res.status,
    durationMs: performance.now() - started,
    correlationId: res.headers.get("x-correlation-id") ?? correlation,
  });
  return res;
}

async function forward(req: NextRequest, path: string, correlation: string) {
  const route = matchRoute(req.method, path);
  if (!route) return error(404, "NotFound", "unknown route", correlation);
  if (!sameOrigin(req.method, req.headers.get("origin"), process.env.APP_BASE_URL ?? "")) {
    return error(403, "CrossOriginRejected", "request origin is not allowed", correlation);
  }
  const base = loanApiBaseUrl();
  if (!base) return error(503, "LoanApiNotConfigured", "LOAN_API_BASE_URL is not set", correlation);

  const headers: Record<string, string> = {
    "x-correlation-id": correlation,
  };
  if (route.auth) {
    if (!auth0) return error(503, "AuthNotConfigured", "sign-in is not configured", correlation);
    try {
      const { token } = await auth0.getAccessToken();
      headers.authorization = `Bearer ${token}`;
    } catch {
      return error(401, "Unauthorized", "sign in to continue", correlation);
    }
  }

  let body: string | undefined;
  if (req.method === "POST") {
    body = sanitizeBody(path, await req.json().catch(() => ({})));
    if (body) headers["content-type"] = "application/json";
  }

  try {
    const res = await fetch(`${base}/api/v1/${path}`, { method: req.method, headers, body, cache: "no-store" });
    const text = await res.text();
    return new NextResponse(text, {
      status: res.status,
      headers: {
        "content-type": res.headers.get("content-type") ?? "application/json",
        "x-correlation-id": res.headers.get("x-correlation-id") ?? headers["x-correlation-id"],
        "cache-control": "no-store",
      },
    });
  } catch {
    return error(502, "LoanApiUnreachable", "the Loan API could not be reached", correlation);
  }
}

export const GET = handle;
export const POST = handle;
