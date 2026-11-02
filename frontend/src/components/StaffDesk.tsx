"use client";

import { useState } from "react";
import { call, describe } from "@/lib/api-client";
import type { ReservationRow } from "@/lib/loan";
import { Notice, type NoticeValue } from "./Notice";
import { ReservationTable, useReservations } from "./ReservationTable";

export function StaffDesk() {
  const { rows, error, load } = useReservations();
  const [filter, setFilter] = useState("");
  const [busy, setBusy] = useState<string | null>(null);
  const [notice, setNotice] = useState<NoticeValue>(null);

  async function act(r: ReservationRow, action: "collect" | "return") {
    setBusy(r.reservationId);
    setNotice(null);
    try {
      await call("POST", `reservations/${r.reservationId}/${action}`);
      setNotice({ kind: "ok", text: `${action === "collect" ? "Collection" : "Return"} recorded for ${r.unitId}. Email queued.` });
    } catch (e) {
      setNotice({ kind: "error", text: describe(e) });
    } finally {
      setBusy(null);
      await load();
    }
  }

  const q = filter.trim().toLowerCase();
  const open = (rows ?? []).filter((r) => r.status !== "RETURNED")
    .filter((r) => !q || [r.reservationId, r.unitId, r.modelId, r.studentEmail].some((v) => v.toLowerCase().includes(q)));

  return (
    <section aria-labelledby="staff-h">
      <h1 id="staff-h">Staff desk: collection and return</h1>
      <label htmlFor="staff-find" className="field-label">Find reservation</label>
      <input id="staff-find" type="search" placeholder="Email, unit, model or reservation ID" value={filter} onChange={(e) => setFilter(e.target.value)} />
      <Notice notice={notice} />
      {error && <p role="alert" className="alert">{error}</p>}
      {rows === null && !error ? <p className="muted" role="status">Loading…</p> : (
        <ReservationTable rows={open} showStudent actions={(r) => r.status === "RESERVED" ? (
          <button className="btn" disabled={busy === r.reservationId} onClick={() => act(r, "collect")}>Mark collected</button>
        ) : (
          <button className="btn" disabled={busy === r.reservationId} onClick={() => act(r, "return")}>Mark returned</button>
        )} />
      )}
    </section>
  );
}
