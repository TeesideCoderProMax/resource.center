"use client";

import Link from "next/link";
import { useRouter } from "next/navigation";
import { AvailabilityPill, monogram, ReserveActions } from "@/components/DeviceCard";
import type { DeviceModelSummary } from "@/lib/loan";

const DAY = 86_400_000;
const WEEKDAYS = ["Mo", "Tu", "We", "Th", "Fr", "Sa", "Su"];

const startOfDay = (d: Date) => new Date(d.getFullYear(), d.getMonth(), d.getDate());
const addDays = (d: Date, n: number) => new Date(d.getFullYear(), d.getMonth(), d.getDate() + n);
const fmtDay = (d: Date) => d.toLocaleDateString("en-GB", { day: "numeric", month: "short" });

/**
 * Month view of the loan window if reserved today. Illustrative only: the API sets the real
 * due time when the reservation is created and the confirmation shows it.
 */
function LoanCalendar({ days }: { days: number }) {
  const today = startOfDay(new Date());
  const due = addDays(today, days);
  const first = new Date(today.getFullYear(), today.getMonth(), 1);
  const lead = (first.getDay() + 6) % 7;
  const monthDays = new Date(today.getFullYear(), today.getMonth() + 1, 0).getDate();
  const cells = Array.from({ length: Math.ceil((lead + monthDays) / 7) * 7 }, (_, i) => addDays(first, i - lead));

  return (
    <div className="calendar">
      <strong className="small">{today.toLocaleDateString("en-GB", { month: "long", year: "numeric" })}</strong>
      <div className="cal-grid" aria-hidden="true">
        {WEEKDAYS.map((w) => <span key={w} className="cal-head">{w}</span>)}
        {cells.map((d) => {
          const t = d.getTime();
          const cls = [
            d.getMonth() !== today.getMonth() && "cal-out",
            t < today.getTime() && "cal-past",
            t === today.getTime() && "cal-start",
            t === due.getTime() && "cal-end",
            t > today.getTime() && t < due.getTime() && "cal-span",
          ].filter(Boolean).join(" ");
          return <span key={t} className={cls}>{d.getDate()}</span>;
        })}
      </div>
      <p className="cal-summary">
        <span>{fmtDay(today)} – {fmtDay(due)}</span>
        <span className="muted small">Exact due time is confirmed when you reserve.</span>
      </p>
    </div>
  );
}

export function DeviceDetail({ item, canAct, signInHref }: { item: DeviceModelSummary; canAct: boolean; signInHref: string | null }) {
  const router = useRouter();
  const available = item.availableUnits > 0;

  return (
    <>
      <nav aria-label="Breadcrumb" className="crumbs">
        <Link href="/">Catalogue</Link><span aria-hidden="true">›</span>
        <span>{item.category}</span><span aria-hidden="true">›</span>
        <span aria-current="page">{item.name}</span>
      </nav>

      <div className="detail-layout">
        <div className="detail-main">
          <article className="glass panel-lg">
            <div className="row row-wrap">
              <AvailabilityPill item={item} />
              <span className="tag">ID {item.id}</span>
            </div>
            <h1>{item.name}</h1>
            <p className="muted">{item.description}</p>
            <div className="showcase" aria-hidden="true"><span className="mono">{monogram(item.name)}</span></div>
          </article>

          <section className="glass panel-lg" aria-labelledby="facts-h">
            <h2 id="facts-h">Loan details</h2>
            <dl className="facts">
              <div><dt>Category</dt><dd>{item.category}</dd></div>
              <div><dt>Loan length</dt><dd>{item.loanDurationDays} days</dd></div>
              <div><dt>Available now</dt><dd className={available ? "good-text" : "warn-text"}>{item.availableUnits} units</dd></div>
              <div><dt>Fleet size</dt><dd>{item.totalUnits} units</dd></div>
            </dl>
          </section>

          <section className="glass panel-lg rules" aria-labelledby="rules-h">
            <h2 id="rules-h">How loans work</h2>
            <ul>
              <li><strong>Who can borrow</strong><span className="muted">Signed-in students and staff.</span></li>
              <li><strong>Collect and return</strong><span className="muted">At the Resource Center desk; staff record each hand-over.</span></li>
              <li><strong>Fully reserved?</strong><span className="muted">Subscribe and you are emailed when a unit comes back.</span></li>
            </ul>
          </section>
        </div>

        <aside className="glass panel-lg reserve-panel" aria-labelledby="reserve-h">
          <span className="eyebrow">Reservation</span>
          <h2 id="reserve-h">{available ? "Reserve this device" : "Currently unavailable"}</h2>
          <div className="row">
            <span className="field-label">Loan window</span>
            <span className="pill pill-accent">{item.loanDurationDays}-day loan</span>
          </div>
          <LoanCalendar days={item.loanDurationDays} />
          <div className="pickup">
            <span className="field-label">Pickup and return</span>
            <strong>Resource Center desk</strong>
            <span className="muted small">Staff mark the device collected and returned.</span>
          </div>
          <div className="reserve-actions">
            {canAct ? (
              <ReserveActions item={item} wide onChanged={() => router.refresh()} />
            ) : signInHref ? (
              <a className="btn primary wide" href={signInHref}>Sign in to reserve or subscribe</a>
            ) : (
              <p className="muted small">Reserving needs a student or staff account.</p>
            )}
          </div>
          <p className="muted small policy">Return the device by its due time; overdue loans are flagged to staff.</p>
        </aside>
      </div>
    </>
  );
}
