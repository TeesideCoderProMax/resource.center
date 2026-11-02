"use client";

import { useCallback, useEffect, useState } from "react";
import type { ApiError, DeviceModelSummary, ReservationRow } from "@/lib/loan";

type Notice = { kind: "ok" | "error"; text: string } | null;

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
};

async function call<T>(method: "GET" | "POST", path: string, body?: unknown): Promise<T> {
  const res = await fetch(`/api/loan/${path}`, {
    method,
    headers: body ? { "content-type": "application/json" } : undefined,
    body: body ? JSON.stringify(body) : method === "POST" ? "{}" : undefined,
    cache: "no-store",
  });
  const data = await res.json().catch(() => ({}));
  if (!res.ok) {
    const err = data as ApiError;
    throw new Error(MESSAGES[err.error] ?? err.message ?? `Request failed (HTTP ${res.status})`);
  }
  return data as T;
}

const fmt = (iso: string | null) => (iso ? new Date(iso).toLocaleString() : "—");

function Banner({ notice }: { notice: Notice }) {
  if (!notice) return null;
  return <p role={notice.kind === "error" ? "alert" : "status"} className={notice.kind === "error" ? "alert" : "ok"}>{notice.text}</p>;
}

export function Catalogue({ initialItems, initialError, signedIn }: { initialItems: DeviceModelSummary[]; initialError: string | null; signedIn: boolean }) {
  const [items, setItems] = useState(initialItems);
  const [busy, setBusy] = useState<string | null>(null);
  const [notice, setNotice] = useState<Notice>(initialError ? { kind: "error", text: initialError } : null);

  const refresh = useCallback(async () => {
    try {
      setItems((await call<{ items: DeviceModelSummary[] }>("GET", "catalogue")).items);
    } catch (e) {
      setNotice({ kind: "error", text: (e as Error).message });
    }
  }, []);

  async function act(item: DeviceModelSummary, action: "reservations" | "subscriptions") {
    setBusy(item.id);
    setNotice(null);
    try {
      if (action === "reservations") {
        const r = await call<{ dueAt: string; unitId: string }>("POST", action, { modelId: item.id });
        setNotice({ kind: "ok", text: `Reserved ${item.name} (unit ${r.unitId}). Return by ${fmt(r.dueAt)}.` });
        window.dispatchEvent(new Event("reservations-changed"));
      } else {
        await call("POST", action, { modelId: item.id });
        setNotice({ kind: "ok", text: `Subscribed. We will email you when ${item.name} is back.` });
      }
    } catch (e) {
      setNotice({ kind: "error", text: (e as Error).message });
    } finally {
      setBusy(null);
      await refresh();
    }
  }

  return (
    <section aria-label="Device catalogue">
      <h2>Devices</h2>
      <Banner notice={notice} />
      <div className="grid">
        {items.map((item) => (
          <article key={item.id} className="card">
            <div className="row">
              <span className="cat">{item.category}</span>
              <span className={`badge ${item.availableUnits > 0 ? "good" : "warn"}`}>
                {item.availableUnits} / {item.totalUnits} available
              </span>
            </div>
            <h3>{item.name}</h3>
            <p className="muted">{item.description}</p>
            <p className="muted small">{item.loanDurationDays}-day loan · {item.id}</p>
            {signedIn && (
              item.availableUnits > 0 ? (
                <button className="btn primary" disabled={busy === item.id} onClick={() => act(item, "reservations")}>
                  {busy === item.id ? "Reserving…" : "Reserve"}
                </button>
              ) : (
                <button className="btn" disabled={busy === item.id} onClick={() => act(item, "subscriptions")}>
                  {busy === item.id ? "Subscribing…" : "Notify me when available"}
                </button>
              )
            )}
          </article>
        ))}
      </div>
      {!signedIn && <p className="muted">Sign in to reserve a device.</p>}
    </section>
  );
}

function useReservations() {
  const [rows, setRows] = useState<ReservationRow[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const load = useCallback(async () => {
    try {
      setRows(await call<ReservationRow[]>("GET", "reservations"));
      setError(null);
    } catch (e) {
      setError((e as Error).message);
    }
  }, []);
  useEffect(() => {
    load();
    const h = () => load();
    window.addEventListener("reservations-changed", h);
    return () => window.removeEventListener("reservations-changed", h);
  }, [load]);
  return { rows, error, load };
}

function Table({ rows, actions }: { rows: ReservationRow[]; actions?: (r: ReservationRow) => React.ReactNode }) {
  if (rows.length === 0) return <p className="muted">No reservations.</p>;
  return (
    <table>
      <thead>
        <tr><th>Device</th><th>Unit</th>{actions && <th>Student</th>}<th>Status</th><th>Reserved</th><th>Due back</th><th>Collected</th><th>Returned</th>{actions && <th />}</tr>
      </thead>
      <tbody>
        {rows.map((r) => (
          <tr key={r.reservationId}>
            <td>{r.modelId}</td><td>{r.unitId}</td>{actions && <td>{r.studentEmail}</td>}
            <td>{r.status}</td><td>{fmt(r.reservedAt)}</td><td><strong>{fmt(r.dueAt)}</strong></td>
            <td>{fmt(r.collectedAt)}</td><td>{fmt(r.returnedAt)}</td>
            {actions && <td>{actions(r)}</td>}
          </tr>
        ))}
      </tbody>
    </table>
  );
}

export function MyReservations({ title }: { title: string }) {
  const { rows, error } = useReservations();
  return (
    <section aria-label={title}>
      <h2>{title}</h2>
      {error && <p role="alert" className="alert">{error}</p>}
      {rows === null && !error ? <p className="muted">Loading…</p> : rows && <Table rows={rows} />}
    </section>
  );
}

export function StaffDesk() {
  const { rows, error, load } = useReservations();
  const [filter, setFilter] = useState("");
  const [busy, setBusy] = useState<string | null>(null);
  const [notice, setNotice] = useState<Notice>(null);

  async function act(r: ReservationRow, action: "collect" | "return") {
    setBusy(r.reservationId);
    setNotice(null);
    try {
      await call("POST", `reservations/${r.reservationId}/${action}`);
      setNotice({ kind: "ok", text: `${action === "collect" ? "Collection" : "Return"} recorded for ${r.unitId}. Email queued.` });
    } catch (e) {
      setNotice({ kind: "error", text: (e as Error).message });
    } finally {
      setBusy(null);
      await load();
      window.dispatchEvent(new Event("reservations-changed"));
    }
  }

  const q = filter.trim().toLowerCase();
  const open = (rows ?? []).filter((r) => r.status !== "RETURNED")
    .filter((r) => !q || [r.reservationId, r.unitId, r.modelId, r.studentEmail].some((v) => v.toLowerCase().includes(q)));

  return (
    <section aria-label="Staff desk">
      <h2>Staff desk: collection and return</h2>
      <input aria-label="Find reservation" placeholder="Find by email, unit, model or reservation ID" value={filter} onChange={(e) => setFilter(e.target.value)} />
      <Banner notice={notice} />
      {error && <p role="alert" className="alert">{error}</p>}
      {rows === null && !error ? <p className="muted">Loading…</p> : (
        <Table rows={open} actions={(r) => r.status === "RESERVED" ? (
          <button className="btn" disabled={busy === r.reservationId} onClick={() => act(r, "collect")}>Mark collected</button>
        ) : (
          <button className="btn" disabled={busy === r.reservationId} onClick={() => act(r, "return")}>Mark returned</button>
        )} />
      )}
    </section>
  );
}
