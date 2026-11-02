import { vi } from "vitest";

type Reply = { status: number; body?: unknown };

/** Stubs fetch for /api/loan/* with a queue of replies per "METHOD path"; records calls. */
export function fakeFetch(routes: Record<string, Reply | Reply[]>) {
  const queues = Object.fromEntries(Object.entries(routes).map(([k, v]) => [k, Array.isArray(v) ? [...v] : [v]]));
  const fn = vi.fn(async (url: string, init?: RequestInit) => {
    const key = `${init?.method ?? "GET"} ${url.replace("/api/loan/", "")}`;
    const q = queues[key];
    if (!q?.length) throw new Error(`unexpected ${key}`);
    const r = q.length > 1 ? q.shift()! : q[0];
    return new Response(r.body === undefined ? "" : JSON.stringify(r.body), {
      status: r.status,
      headers: { "x-correlation-id": "web-test" },
    });
  });
  vi.stubGlobal("fetch", fn);
  return fn;
}
