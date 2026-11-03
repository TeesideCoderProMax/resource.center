"use client";

import { useCallback, useEffect, useState } from "react";
import { call, fmt } from "@/lib/api-client";
import type { ReservationRow } from "@/lib/loan";
import { errorNotice, Notice, type NoticeValue } from "./Notice";

/** Overdue: due date passed and the device is not back yet. */
export const isOverdue = (r: ReservationRow, now = Date.now()) => r.status !== "RETURNED" && new Date(r.dueAt).getTime() < now;

export function useReservations(returnTo: string) {
  const [rows, setRows] = useState<ReservationRow[] | null>(null);
  const [error, setError] = useState<NoticeValue>(null);
  const load = useCallback(async () => {
    try {
      setRows(await call<ReservationRow[]>("GET", "reservations"));
      setError(null);
    } catch (e) {
      setError(errorNotice(e, returnTo));
    }
  }, [returnTo]);
  useEffect(() => { load(); }, [load]);
  return { rows, error, load };
}

export function ReservationTable({ rows, showStudent = false, actions }: {
  rows: ReservationRow[];
  showStudent?: boolean;
  actions?: (r: ReservationRow) => React.ReactNode;
}) {
  if (rows.length === 0) return <p className="muted">No reservations.</p>;
  const now = Date.now();
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
          {rows.map((r) => {
            const overdue = isOverdue(r, now);
            return (
              <tr key={r.reservationId} className={overdue ? "overdue" : undefined}>
                <td>{r.modelId}</td><td>{r.unitId}</td>{showStudent && <td>{r.studentEmail}</td>}
                <td>
                  <span className={`status s-${r.status.toLowerCase()}`}>{r.status}</span>
                  {overdue && <> <span className="badge overdue-badge">Overdue</span></>}
                </td>
                <td>{fmt(r.reservedAt)}</td><td><strong>{fmt(r.dueAt)}</strong></td>
                <td>{fmt(r.collectedAt)}</td><td>{fmt(r.returnedAt)}</td>
                {actions && <td>{actions(r)}</td>}
              </tr>
            );
          })}
        </tbody>
      </table>
    </div>
  );
}

export function MyReservations({ title, showStudent = false }: { title: string; showStudent?: boolean }) {
  const { rows, error } = useReservations("/reservations");
  return (
    <section aria-labelledby="my-res-h">
      <div className="page-head">
        <div>
          <span className="eyebrow">{showStudent ? "Staff view" : "Your loans"}</span>
          <h1 id="my-res-h">{title}</h1>
          <p className="muted">{showStudent ? "Every borrower's reservations, newest first." : "Collect reserved devices at the desk and return them by the due date."}</p>
        </div>
      </div>
      <Notice notice={error} />
      {rows === null && !error ? <p className="muted" role="status">Loading…</p> : rows && <ReservationTable rows={rows} showStudent={showStudent} />}
    </section>
  );
}
