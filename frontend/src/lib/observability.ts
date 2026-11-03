/**
 * Web-tier observability helpers (pure, no Next.js imports). Logs are one JSON object per
 * line on stdout for Log Analytics. Never log tokens, emails, bodies or raw paths with IDs.
 */
const SAFE_CORRELATION = /^[A-Za-z0-9._:-]{1,128}$/;
const UUID = /[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}/g;

/** Forward a caller's correlation ID only when it is a short safe token; otherwise mint one. */
export function correlationId(incoming: string | null): string {
  return incoming && SAFE_CORRELATION.test(incoming) ? incoming : `web-${crypto.randomUUID()}`;
}

/** Route template for logs: `reservations/{id}/collect`, so IDs never reach log fields. */
export function routeTemplate(path: string): string {
  return path.replace(UUID, "{id}").slice(0, 100);
}

export interface ProxyLog {
  method: string;
  route: string;
  status: number;
  durationMs: number;
  correlationId: string;
}

export function logProxy(entry: ProxyLog, write: (line: string) => void = console.log): void {
  write(
    JSON.stringify({
      timestamp: new Date().toISOString(),
      level: entry.status >= 500 ? "error" : entry.status >= 400 ? "warn" : "info",
      service: "resource-center-web",
      event: "loan_proxy",
      method: entry.method,
      route: entry.route,
      status: entry.status,
      duration_ms: Math.round(entry.durationMs),
      correlation_id: entry.correlationId,
    })
  );
}
