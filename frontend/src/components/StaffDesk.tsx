"use client";

import Link from "next/link";
import { useMemo, useState } from "react";
import { call, fmt } from "@/lib/api-client";
import type { DeviceModelSummary, ReservationRow } from "@/lib/loan";
import { categoryIcon, Icon, type IconName } from "./Icon";
import { errorNotice, Notice, type NoticeValue } from "./Notice";
import { isOverdue, ReservationTable, useReservations } from "./ReservationTable";

type Action = "collect" | "return";
type Status = "OPEN" | ReservationRow["status"] | "ALL";

const actionFor = (r: ReservationRow): Action | null =>
  r.status === "RESERVED" ? "collect" : r.status === "COLLECTED" ? "return" : null;

const initials = (email: string) => email.split("@")[0].split(/[._-]/).slice(0, 2).map((p) => p[0]?.toUpperCase() ?? "").join("") || "?";

/** Desk telemetry derived only from the catalogue and the reservation list (newest 200 rows). */
export function deskStats(rows: ReservationRow[], catalogue: DeviceModelSummary[], now = Date.now()) {
  const totalUnits = catalogue.reduce((n, m) => n + m.totalUnits, 0);
  const availableUnits = catalogue.reduce((n, m) => n + m.availableUnits, 0);
  const returned = rows.filter((r) => r.status === "RETURNED" && r.returnedAt);
  const onTime = returned.filter((r) => new Date(r.returnedAt!).getTime() <= new Date(r.dueAt).getTime()).length;
  return {
    totalUnits,
    inUsePct: totalUnits ? Math.round(((totalUnits - availableUnits) / totalUnits) * 100) : 0,
    awaiting: rows.filter((r) => r.status === "RESERVED").length,
    active: rows.filter((r) => r.status === "COLLECTED").length,
    onTimePct: returned.length ? Math.round((onTime / returned.length) * 100) : null,
    overdue: rows.filter((r) => isOverdue(r, now)).length,
  };
}

function csv(rows: ReservationRow[]): string {
  const cols: (keyof ReservationRow)[] = ["reservationId", "modelId", "unitId", "studentEmail", "status", "reservedAt", "dueAt", "collectedAt", "returnedAt"];
  const cell = (v: unknown) => `"${String(v ?? "").replaceAll('"', '""')}"`;
  return [cols.join(","), ...rows.map((r) => cols.map((c) => cell(r[c])).join(","))].join("\n");
}

function download(rows: ReservationRow[]) {
  const url = URL.createObjectURL(new Blob([csv(rows)], { type: "text/csv" }));
  const a = Object.assign(document.createElement("a"), { href: url, download: `reservations-${new Date().toISOString().slice(0, 10)}.csv` });
  a.click();
  URL.revokeObjectURL(url);
}

function StatCard({ icon, label, value, unit, foot, tone }: { icon: IconName; label: string; value: number | string; unit: string; foot: React.ReactNode; tone?: "accent" | "danger" }) {
  return (
    <div className={`glass stat-card${tone ? ` ${tone}` : ""}`}>
      <div className="row"><span className="muted small">{label}</span><span className="stat-icon"><Icon name={icon} /></span></div>
      <p className="stat-value"><strong>{value}</strong> <span className="muted">{unit}</span></p>
      <div className="small">{foot}</div>
    </div>
  );
}

export function StaffDesk({ catalogue = [] }: { catalogue?: DeviceModelSummary[] }) {
  const { rows, error, load } = useReservations("/staff");
  const [filter, setFilter] = useState("");
  const [status, setStatus] = useState<Status>("OPEN");
  const [pending, setPending] = useState<{ id: string; action: Action } | null>(null);
  const [busy, setBusy] = useState<string | null>(null);
  const [notice, setNotice] = useState<NoticeValue>(null);
  const [invQuery, setInvQuery] = useState("");
  const [invCategory, setInvCategory] = useState("");

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

  const all = rows ?? [];
  const now = Date.now();
  const stats = deskStats(all, catalogue, now);
  const names = useMemo(() => new Map(catalogue.map((m) => [m.id, m.name])), [catalogue]);
  const queue = all.filter((r) => r.status === "RESERVED");
  const q = filter.trim().toLowerCase();
  const shown = all
    .filter((r) => status === "ALL" || (status === "OPEN" ? r.status !== "RETURNED" : r.status === status))
    .filter((r) => !q || [r.reservationId, r.unitId, r.modelId, r.studentEmail].some((v) => v.toLowerCase().includes(q)));
  const iq = invQuery.trim().toLowerCase();
  const inventory = catalogue
    .filter((m) => !invCategory || m.category === invCategory)
    .filter((m) => !iq || `${m.id} ${m.name} ${m.description}`.toLowerCase().includes(iq));
  const categories = [...new Set(catalogue.map((m) => m.category))].sort();

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
    <div className="admin-layout">
      <aside className="admin-side glass" aria-label="Staff sections">
        <div className="side-head"><Icon name="shield" /><div><strong className="block">Staff portal</strong><span className="muted small">Resource Center desk</span></div></div>
        <ul>
          <li><a href="#dashboard"><Icon name="dashboard" /> Dashboard</a></li>
          <li><a href="#queue"><Icon name="clock" /> Collection queue {queue.length > 0 && <span className="count">{queue.length}</span>}</a></li>
          <li><a href="#loans"><Icon name="swap" /> Loans</a></li>
          <li><a href="#loans" onClick={() => setStatus("OPEN")}><Icon name="alert" /> Overdue {stats.overdue > 0 && <span className="count danger">{stats.overdue}</span>}</a></li>
          <li><a href="#inventory"><Icon name="grid" /> Inventory</a></li>
        </ul>
      </aside>

      <div className="admin-main">
        <section aria-labelledby="staff-h" id="dashboard" className="page-head">
          <div>
            <nav aria-label="Breadcrumb" className="crumbs"><Link href="/">Home</Link><span aria-hidden="true">›</span><span aria-current="page" className="accent">Staff Admin</span></nav>
            <h1 id="staff-h">Staff desk: collection and return</h1>
            <p className="muted">Hand devices over, record returns, chase overdue loans and check stock.</p>
          </div>
          <div className="row-start">
            <button className="btn" onClick={() => load()}><Icon name="refresh" /> Refresh</button>
            <button className="btn primary" disabled={!all.length} onClick={() => download(shown)}><Icon name="download" /> Export CSV</button>
          </div>
        </section>

        <div className="stat-grid" aria-label="Desk summary">
          <StatCard icon="laptop" label="Total equipment" value={stats.totalUnits} unit="units"
            foot={<><span className="bar"><span style={{ width: `${stats.inUsePct}%` }} /></span> <span className="accent">{stats.inUsePct}% in use</span></>} />
          <StatCard icon="clock" label="Awaiting collection" value={stats.awaiting} unit="reserved" tone="accent" foot={<span className="muted">Reserved, not yet handed over</span>} />
          <StatCard icon="swap" label="Active loans out" value={stats.active} unit="collected"
            foot={stats.onTimePct === null ? <span className="muted">No returns yet</span> : <span className="good-text"><Icon name="check" size={14} /> {stats.onTimePct}% on-time returns</span>} />
          <StatCard icon="alert" label="Overdue" value={stats.overdue} unit="loans" tone={stats.overdue ? "danger" : undefined} foot={<span className="muted">Past due and not returned</span>} />
        </div>

        <Notice notice={notice} />
        <Notice notice={error} />

        <section aria-labelledby="queue-h" id="queue">
          <div className="row">
            <div><h2 id="queue-h" className="h-sm"><Icon name="clock" /> Collection queue</h2><p className="muted small">Reserved devices waiting for hand-over at the desk.</p></div>
            {queue.length > 0 && <span className="pill pill-accent">{queue.length} waiting</span>}
          </div>
          {rows === null && !error ? <p className="muted" role="status">Loading…</p> : queue.length === 0 ? <p className="muted small">Nobody is waiting to collect.</p> : (
            <div className="queue">
              {queue.map((r) => (
                <article key={r.reservationId} className="glass queue-card">
                  <span className="avatar lg" aria-hidden="true">{initials(r.studentEmail)}</span>
                  <div className="queue-body">
                    <div className="row-start"><strong>{r.studentEmail}</strong>{isOverdue(r, now) && <span className="badge overdue-badge">Overdue</span>}</div>
                    <span className="accent"><Icon name={categoryIcon(catalogue.find((m) => m.id === r.modelId)?.category ?? "")} size={16} /> {names.get(r.modelId) ?? r.modelId}</span>
                    <span className="muted small"><Icon name="calendar" size={14} /> Reserved {fmt(r.reservedAt)} · due {fmt(r.dueAt)}</span>
                    <span className="small">Unit: <span className="tag">{r.unitId}</span></span>
                  </div>
                </article>
              ))}
            </div>
          )}
        </section>

        <section aria-labelledby="loans-h" id="loans" className="glass panel-lg">
          <h2 id="loans-h" className="h-sm"><Icon name="swap" /> Loans</h2>
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
          </div>
          {rows === null && !error ? <p className="muted">Loading…</p> : (
            <ReservationTable rows={shown} showStudent actions={actions} />
          )}
        </section>

        <section aria-labelledby="inv-h" id="inventory" className="glass panel-lg">
          <div className="row">
            <div><h2 id="inv-h" className="h-sm"><Icon name="box" /> Equipment inventory</h2><p className="muted small">Live stock per model from the catalogue.</p></div>
          </div>
          <div className="filters">
            <div className="grow">
              <label htmlFor="inv-find" className="field-label">Filter models</label>
              <input id="inv-find" type="search" placeholder="Model ID, name or description" value={invQuery} onChange={(e) => setInvQuery(e.target.value)} />
            </div>
            <div>
              <label htmlFor="inv-cat" className="field-label">Category</label>
              <select id="inv-cat" value={invCategory} onChange={(e) => setInvCategory(e.target.value)}>
                <option value="">All categories</option>
                {categories.map((c) => <option key={c} value={c}>{c}</option>)}
              </select>
            </div>
          </div>
          {catalogue.length === 0 ? <p className="muted small">Catalogue unavailable.</p> : (
            <div className="table-wrap">
              <table aria-label="Equipment inventory">
                <thead><tr><th scope="col">Model ID</th><th scope="col">Equipment</th><th scope="col">Category</th><th scope="col">Stock</th><th scope="col">Status</th></tr></thead>
                <tbody>
                  {inventory.map((m) => {
                    const pct = m.totalUnits ? Math.round((m.availableUnits / m.totalUnits) * 100) : 0;
                    return (
                      <tr key={m.id}>
                        <td><span className="tag">{m.id}</span></td>
                        <td><strong className="block">{m.name}</strong><span className="muted small">{m.description} · {m.loanDurationDays}-day loan</span></td>
                        <td><span className="pill pill-plain"><Icon name={categoryIcon(m.category)} size={14} /> {m.category}</span></td>
                        <td>
                          <strong>{m.availableUnits}</strong> <span className="muted small">/ {m.totalUnits} available</span>
                          <span className={`bar${m.availableUnits === 0 ? " danger" : m.availableUnits === 1 ? " warn" : ""}`}><span style={{ width: `${pct}%` }} /></span>
                        </td>
                        <td><span className={`pill ${m.availableUnits > 0 ? "pill-in" : "pill-out"}`}>{m.availableUnits > 0 ? "Available" : "Waitlist"}</span></td>
                      </tr>
                    );
                  })}
                </tbody>
              </table>
            </div>
          )}
        </section>
      </div>
    </div>
  );
}
