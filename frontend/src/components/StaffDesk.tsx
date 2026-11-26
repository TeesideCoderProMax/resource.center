"use client";

import Link from "next/link";
import { useEffect, useMemo, useState } from "react";
import { call, fmt } from "@/lib/api-client";
import type { DeviceModelSummary, ReservationRow } from "@/lib/loan";
import { categoryIcon, Icon, type IconName } from "./Icon";
import { errorNotice, Notice, type NoticeValue } from "./Notice";
import { isOverdue, ReservationTable, useReservations } from "./ReservationTable";
import { StaffSidebar } from "./StaffSidebar";

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
  const { rows, error, load } = useReservations("/staff/dashboard");
  const [filter, setFilter] = useState("");
  const [status, setStatus] = useState<Status>("OPEN");
  const [pending, setPending] = useState<{ id: string; action: Action } | null>(null);
  const [busy, setBusy] = useState<string | null>(null);
  const [notice, setNotice] = useState<NoticeValue>(null);
  const [section, setSection] = useState("dashboard");

  useEffect(() => {
    const syncHash = () => {
      const hash = window.location.hash.slice(1);
      if (hash === "overdue") {
        setStatus("OPEN");
        setSection("overdue");
        document.getElementById("loans")?.scrollIntoView?.();
      } else if (hash === "queue" || hash === "loans" || hash === "dashboard") {
        setSection(hash);
      }
    };
    syncHash();
    window.addEventListener("hashchange", syncHash);

    // Scroll-spy: the section nearest the top (below the sticky header) is active.
    const ids = ["dashboard", "queue", "loans"];
    const syncScroll = () => {
      const atBottom = innerHeight + scrollY >= document.documentElement.scrollHeight - 4;
      const top = atBottom ? ids.at(-1)! : ids.filter((id) => (document.getElementById(id)?.getBoundingClientRect().top ?? Infinity) <= 140).at(-1) ?? ids[0];
      setSection((cur) => (cur === "overdue" && top === "loans" ? cur : top));
    };
    window.addEventListener("scroll", syncScroll, { passive: true });
    return () => {
      window.removeEventListener("hashchange", syncHash);
      window.removeEventListener("scroll", syncScroll);
    };
  }, []);

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
      const n = errorNotice(e, "/staff/dashboard");
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
      <StaffSidebar
        activePage="dashboard"
        activeSection={section}
        onSelectSection={(key) => {
          if (key === "overdue") setStatus("OPEN");
          setSection(key);
        }}
        queueCount={queue.length}
        overdueCount={stats.overdue}
        inventoryCount={catalogue.length || undefined}
      />

      <div className="admin-main">
        <section aria-labelledby="staff-h" id="dashboard" className="page-head">
          <div>
            <nav aria-label="Breadcrumbs" className="crumbs">
              <ol>
                <li><Link href="/"><Icon name="home" size={14} /> Catalog</Link></li>
                <li><Link href="/staff/dashboard">Staff Admin</Link></li>
                <li><Link href="/staff/dashboard" aria-current="page" className="accent">Dashboard</Link></li>
              </ol>
            </nav>
            <h1 id="staff-h">Staff desk: collection and return</h1>
            <p className="muted">Hand devices over, record returns, chase overdue loans and track live desk queues.</p>
          </div>
          <div className="row-start">
            <button className="btn" onClick={() => load()}><Icon name="refresh" /> Refresh</button>
            <button className="btn primary" disabled={!all.length} onClick={() => download(shown)}><Icon name="download" /> Export CSV</button>
          </div>
        </section>

        <div className="stat-grid" role="region" aria-label="Desk summary">
          <StatCard icon="laptop" label="Total equipment" value={stats.totalUnits} unit="units"
            foot={<><span className="bar"><span style={{ width: `${stats.inUsePct}%` }} /></span> <span className="accent">{stats.inUsePct}% in use</span></>} />
          <StatCard icon="clock" label="Awaiting collection" value={stats.awaiting} unit="reserved" tone="accent" foot={<span className="muted">Reserved, not yet handed over</span>} />
          <StatCard icon="swap" label="Active loans out" value={stats.active} unit="collected"
            foot={stats.onTimePct === null ? <span className="muted">No returns yet</span> : <span className="good-text"><Icon name="check" size={14} /> {stats.onTimePct}% on-time returns</span>} />
          <StatCard icon="alert" label="Overdue" value={stats.overdue} unit="loans" tone={stats.overdue ? "danger" : undefined} foot={<span className="muted">Past due and not returned</span>} />
        </div>

        <Notice notice={notice} />
        <Notice notice={error} />

        <section aria-labelledby="queue-h" id="queue" className="glass panel-lg">
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
          <span id="overdue" className="scroll-anchor" aria-hidden="true" />
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
      </div>
    </div>
  );
}
