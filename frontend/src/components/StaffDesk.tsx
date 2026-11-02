"use client";

import { useState } from "react";
import { call, fmt } from "@/lib/api-client";
import type { ReservationRow } from "@/lib/loan";
import { errorNotice, Notice, type NoticeValue } from "./Notice";
import { ReservationTable, useReservations } from "./ReservationTable";

type Action = "collect" | "return";
type Status = "OPEN" | ReservationRow["status"] | "ALL";

const actionFor = (r: ReservationRow): Action | null =>
  r.status === "RESERVED" ? "collect" : r.status === "COLLECTED" ? "return" : null;

export function StaffDesk() {
  const { rows, error, load } = useReservations("/staff");
  const [filter, setFilter] = useState("");
  const [status, setStatus] = useState<Status>("OPEN");
  const [pending, setPending] = useState<{ id: string; action: Action } | null>(null);
  const [busy, setBusy] = useState<string | null>(null);
  const [notice, setNotice] = useState<NoticeValue>(null);

  async function confirm(r: ReservationRow, action: Action) {
    if (busy) return;
    setBusy(r.reservationId);
    setNotice(null);
    try {
      const res = await call<{ collectedAt?: string; returnedAt?: string }>("POST", `reservations/${r.reservationId}/${action}`);
      setNotice(action === "collect"
        ? { kind: "ok", text: `Collected ${r.unitId} at ${fmt(res.collectedAt ?? null)} for ${r.studentEmail}. Due back ${fmt(r.dueAt)}. Confirmation email queued.` }
        : { kind: "ok", text: `Returned ${r.unitId} at ${fmt(res.returnedAt ?? null)}. Return email queued; subscribers will be notified.` });
    } catch (e) {
      const n = errorNotice(e, "/staff");
      // 409 InvalidTransition: someone else changed the row; the reload below shows its real state.
      setNotice(n?.hint === "refresh" ? { ...n, text: `${n.text} The list has been refreshed.` } : n);
    } finally {
      setBusy(null);
      setPending(null);
      await load();
    }
  }

  const q = filter.trim().toLowerCase();
  const shown = (rows ?? [])
    .filter((r) => status === "ALL" || (status === "OPEN" ? r.status !== "RETURNED" : r.status === status))
    .filter((r) => !q || [r.reservationId, r.unitId, r.modelId, r.studentEmail].some((v) => v.toLowerCase().includes(q)));

  function actions(r: ReservationRow) {
    const action = actionFor(r);
    if (!action) return null;
    const label = action === "collect" ? "Mark collected" : "Mark returned";
    if (pending?.id !== r.reservationId) {
      return <button className="btn" disabled={busy !== null} onClick={() => setPending({ id: r.reservationId, action })}>{label}</button>;
    }
    const isBusy = busy === r.reservationId;
    return (
      <span className="confirm" role="group" aria-label={`Confirm ${action} of ${r.unitId}`}>
        <button className="btn primary" disabled={isBusy} aria-busy={isBusy} onClick={() => confirm(r, action)}>
          {isBusy ? "Saving…" : `Confirm ${action}`}
        </button>{" "}
        <button className="btn" disabled={isBusy} onClick={() => setPending(null)}>Cancel</button>
      </span>
    );
  }

  return (
    <section aria-labelledby="staff-h">
      <h1 id="staff-h">Staff desk: collection and return</h1>
      <div className="filters">
        <div>
          <label htmlFor="staff-status" className="field-label">Status</label>
          <select id="staff-status" value={status} onChange={(e) => setStatus(e.target.value as Status)}>
            <option value="OPEN">Open (reserved or collected)</option>
            <option value="RESERVED">Reserved</option>
            <option value="COLLECTED">Collected</option>
            <option value="RETURNED">Returned</option>
            <option value="ALL">All</option>
          </select>
        </div>
        <div className="grow">
          <label htmlFor="staff-find" className="field-label">Find reservation</label>
          <input id="staff-find" type="search" placeholder="Email, unit, model or reservation ID" value={filter} onChange={(e) => setFilter(e.target.value)} />
        </div>
        <button className="btn" onClick={() => load()}>Refresh</button>
      </div>
      <Notice notice={notice} />
      <Notice notice={error} />
      {rows === null && !error ? <p className="muted" role="status">Loading…</p> : (
        <ReservationTable rows={shown} showStudent actions={actions} />
      )}
    </section>
  );
}
