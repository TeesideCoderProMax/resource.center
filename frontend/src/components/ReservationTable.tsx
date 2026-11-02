"use client";

import { useCallback, useEffect, useState } from "react";
import { call, describe, fmt } from "@/lib/api-client";
import type { ReservationRow } from "@/lib/loan";

export function useReservations() {
  const [rows, setRows] = useState<ReservationRow[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const load = useCallback(async () => {
    try {
      setRows(await call<ReservationRow[]>("GET", "reservations"));
      setError(null);
    } catch (e) {
      setError(describe(e));
    }
  }, []);
  useEffect(() => { load(); }, [load]);
  return { rows, error, load };
}

export function ReservationTable({ rows, showStudent = false, actions }: {
  rows: ReservationRow[];
  showStudent?: boolean;
  actions?: (r: ReservationRow) => React.ReactNode;
}) {
  if (rows.length === 0) return <p className="muted">No reservations.</p>;
  return (
    <div className="table-wrap">
      <table>
        <thead>
          <tr>
            <th scope="col">Device</th><th scope="col">Unit</th>{showStudent && <th scope="col">Student</th>}
            <th scope="col">Status</th><th scope="col">Reserved</th><th scope="col">Due back</th>
            <th scope="col">Collected</th><th scope="col">Returned</th>{actions && <th scope="col"><span className="sr-only">Action</span></th>}
          </tr>
        </thead>
        <tbody>
          {rows.map((r) => (
            <tr key={r.reservationId}>
              <td>{r.modelId}</td><td>{r.unitId}</td>{showStudent && <td>{r.studentEmail}</td>}
              <td><span className={`status s-${r.status.toLowerCase()}`}>{r.status}</span></td>
              <td>{fmt(r.reservedAt)}</td><td><strong>{fmt(r.dueAt)}</strong></td>
              <td>{fmt(r.collectedAt)}</td><td>{fmt(r.returnedAt)}</td>
              {actions && <td>{actions(r)}</td>}
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}

export function MyReservations({ title, showStudent = false }: { title: string; showStudent?: boolean }) {
  const { rows, error } = useReservations();
  return (
    <section aria-labelledby="my-res-h">
      <h1 id="my-res-h">{title}</h1>
      {error && <p role="alert" className="alert">{error}</p>}
      {rows === null && !error ? <p className="muted" role="status">Loading…</p> : rows && <ReservationTable rows={rows} showStudent={showStudent} />}
    </section>
  );
}
