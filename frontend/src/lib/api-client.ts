/** Browser client for the /api/loan BFF proxy. Never sees tokens; the proxy adds them. */
import type { ApiError } from "./loan";

const MESSAGES: Record<string, string> = {
  NoAvailableUnits: "No units left. Someone took the last one. You can subscribe instead.",
  UnitsCurrentlyAvailable: "Units are available now, so you can reserve one instead.",
  InvalidTransition: "That reservation is not in the right state for this action.",
  ForbiddenRole: "Your role does not allow this action.",
  NotFound: "Not found.",
  Unauthorized: "Please sign in again.",
  InvalidToken: "Your session is not valid. Please sign in again.",
  AuthNotConfigured: "Sign-in is not configured on this environment.",
  AuthUnavailable: "Sign-in keys are unavailable. Try again shortly.",
  InvalidInput: "The request was not valid.",
  CrossOriginRejected: "The request came from an unexpected origin. Reload the page.",
  LoanApiNotConfigured: "The Loan API is not configured on this environment.",
  LoanApiUnreachable: "The Loan API could not be reached. Try again shortly.",
};

export function errorMessage(status: number, body: Partial<ApiError> | null): string {
  return (body?.error && MESSAGES[body.error]) || body?.message || `Request failed (HTTP ${status})`;
}

export class ApiClientError extends Error {
  constructor(message: string, readonly status: number, readonly code: string | undefined, readonly correlationId: string | null) {
    super(message);
  }
}

export async function call<T>(method: "GET" | "POST", path: string, body?: unknown): Promise<T> {
  const res = await fetch(`/api/loan/${path}`, {
    method,
    headers: body ? { "content-type": "application/json" } : undefined,
    body: body ? JSON.stringify(body) : method === "POST" ? "{}" : undefined,
    cache: "no-store",
  });
  const data = await res.json().catch(() => null);
  if (!res.ok) {
    const err = data as Partial<ApiError> | null;
    const correlationId = res.headers.get("x-correlation-id") ?? err?.correlationId ?? null;
    throw new ApiClientError(errorMessage(res.status, err), res.status, err?.error, correlationId);
  }
  return data as T;
}

/** Message for a caught error, with the correlation ID appended for support. */
export function describe(e: unknown): string {
  if (e instanceof ApiClientError) return e.correlationId ? `${e.message} (ref ${e.correlationId})` : e.message;
  return e instanceof Error ? e.message : String(e);
}

export const fmt = (iso: string | null) => (iso ? new Date(iso).toLocaleString() : "—");
