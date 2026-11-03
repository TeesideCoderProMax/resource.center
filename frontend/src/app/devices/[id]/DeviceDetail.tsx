"use client";

import Link from "next/link";
import { useRouter } from "next/navigation";
import { useState } from "react";
import { AddToBag, AvailabilityPill, DevicePreview, ReserveActions } from "@/components/DeviceCard";
import { Icon, type IconName } from "@/components/Icon";
import type { DeviceModelSummary } from "@/lib/loan";

const WEEKDAYS = ["Mo", "Tu", "We", "Th", "Fr", "Sa", "Su"];

const startOfDay = (d: Date) => new Date(d.getFullYear(), d.getMonth(), d.getDate());
const addDays = (d: Date, n: number) => new Date(d.getFullYear(), d.getMonth(), d.getDate() + n);
const fmtDay = (d: Date) => d.toLocaleDateString("en-GB", { day: "numeric", month: "short" });

/**
 * Month view of the loan window if reserved today. Illustrative only: reservations start
 * immediately and the API sets the real due time, which the confirmation shows.
 */
function LoanCalendar({ days }: { days: number }) {
  const today = startOfDay(new Date());
  const due = addDays(today, days);
  const [offset, setOffset] = useState(0);
  const first = new Date(today.getFullYear(), today.getMonth() + offset, 1);
  const lead = (first.getDay() + 6) % 7;
  const monthDays = new Date(first.getFullYear(), first.getMonth() + 1, 0).getDate();
  const cells = Array.from({ length: Math.ceil((lead + monthDays) / 7) * 7 }, (_, i) => addDays(first, i - lead));

  return (
    <div className="calendar">
      <div className="row">
        <strong className="small">{first.toLocaleDateString("en-GB", { month: "long", year: "numeric" })}</strong>
        <span className="row-start">
          <button className="icon-btn sm" disabled={offset === 0} onClick={() => setOffset(offset - 1)} aria-label="Previous month"><Icon name="chevronLeft" size={16} /></button>
          <button className="icon-btn sm" disabled={offset >= 1} onClick={() => setOffset(offset + 1)} aria-label="Next month"><Icon name="chevronRight" size={16} /></button>
        </span>
      </div>
      <div className="cal-grid" aria-hidden="true">
        {WEEKDAYS.map((w) => <span key={w} className="cal-head">{w}</span>)}
        {cells.map((d) => {
          const t = d.getTime();
          const cls = [
            d.getMonth() !== first.getMonth() && "cal-out",
            t < today.getTime() && "cal-past",
            t === today.getTime() && "cal-start",
            t === due.getTime() && "cal-end",
            t > today.getTime() && t < due.getTime() && "cal-span",
          ].filter(Boolean).join(" ");
          return <span key={t} className={cls}>{d.getDate()}</span>;
        })}
      </div>
      <p className="cal-summary">
        <span><span className="dot accent-dot" aria-hidden="true" /> {fmtDay(today)} – {fmtDay(due)}</span>
        <span className="muted small">Starts when you reserve; exact due time is confirmed then.</span>
      </p>
    </div>
  );
}

function Fact({ icon, label, value, tone }: { icon: IconName; label: string; value: string; tone?: string }) {
  return (
    <div>
      <dt><Icon name={icon} size={16} /> {label}</dt>
      <dd className={tone}>{value}</dd>
    </div>
  );
}

export function DeviceDetail({ item, canAct, signInHref }: { item: DeviceModelSummary; canAct: boolean; signInHref: string | null }) {
  const router = useRouter();
  const available = item.availableUnits > 0;

  return (
    <>
      <nav aria-label="Breadcrumb" className="crumbs">
        <Link href="/"><Icon name="home" size={14} /> Home</Link><span aria-hidden="true">›</span>
        <Link href="/">Catalog</Link><span aria-hidden="true">›</span>
        <span className="cap">{item.category}</span><span aria-hidden="true">›</span>
        <span aria-current="page">{item.name}</span>
      </nav>

      <div className="detail-layout">
        <div className="detail-main">
          <article className="glass panel-lg">
            <div className="row row-wrap">
              <span className={`status-pill ${available ? "status-ok" : "status-wait"}`}>
                <span className="pulse" aria-hidden="true" />
                {available ? "Available for immediate reservation" : "Fully reserved: join the waitlist"}
              </span>
              <span className="tag">MODEL {item.id}</span>
            </div>
            <h1>{item.name}</h1>
            <p className="muted">{item.description}</p>
            <DevicePreview item={item} large />
            <div className="pills"><AvailabilityPill item={item} /><span className="pill pill-plain">{item.category}</span></div>
          </article>

          <section className="glass panel-lg" aria-labelledby="facts-h">
            <div className="row">
              <h2 id="facts-h"><Icon name="chip" /> Device facts</h2>
              <span className="pill pill-accent">From the live catalogue</span>
            </div>
            <dl className="facts">
              <Fact icon="box" label="Category" value={item.category} />
              <Fact icon="clock" label="Loan length" value={`${item.loanDurationDays} days`} />
              <Fact icon="check" label="Available now" value={`${item.availableUnits} units`} tone={available ? "good-text" : "warn-text"} />
              <Fact icon="grid" label="Fleet size" value={`${item.totalUnits} units`} />
              <Fact icon="pin" label="Collection" value="Resource Center desk" />
              <Fact icon="shield" label="Hand-over" value="Recorded by staff" />
            </dl>
          </section>

          <section className="glass panel-lg rules" aria-labelledby="rules-h">
            <h2 id="rules-h"><Icon name="shield" /> Eligibility &amp; loan rules</h2>
            <ul>
              <li><span className="rule-icon"><Icon name="user" /></span><div><strong>Who can borrow</strong><span className="muted">Signed-in students and staff.</span></div></li>
              <li><span className="rule-icon"><Icon name="clock" /></span><div><strong>Maximum loan</strong><span className="muted">{item.loanDurationDays} days from reservation.</span></div></li>
              <li><span className="rule-icon"><Icon name="bag" /></span><div><strong>Waitlist</strong><span className="muted">Subscribe when fully reserved; you are emailed on return.</span></div></li>
            </ul>
          </section>
        </div>

        <aside className="glass panel-lg reserve-panel" aria-labelledby="reserve-h">
          <div className="row">
            <div>
              <span className="eyebrow">Instant loan</span>
              <h2 id="reserve-h">Configure reservation</h2>
            </div>
            <span className="accent"><Icon name="calendar" size={26} /></span>
          </div>
          <div className="row">
            <span className="field-label"><Icon name="calendar" size={16} /> Loan window</span>
            <span className="pill pill-accent">{item.loanDurationDays} days duration</span>
          </div>
          <LoanCalendar days={item.loanDurationDays} />
          <div>
            <span className="field-label"><Icon name="pin" size={16} /> Pickup &amp; return hub</span>
            <div className="pickup">
              <div className="row">
                <strong>Resource Center desk</strong>
                <span className="pill pill-in">Staffed</span>
              </div>
              <span className="muted small">Show your campus account; staff mark the device collected and returned.</span>
            </div>
          </div>
          <div className="reserve-actions">
            {canAct ? (
              <>
                <AddToBag item={item} wide />
                <ReserveActions item={item} wide onChanged={() => router.refresh()} />
              </>
            ) : signInHref ? (
              <a className="btn primary wide" href={signInHref}>Sign in to reserve or subscribe</a>
            ) : (
              <p className="muted small">Reserving needs a student or staff account.</p>
            )}
          </div>
          <p className="muted small policy"><Icon name="warning" size={14} /> Return the device by its due time; overdue loans are flagged to desk staff.</p>
        </aside>
      </div>
    </>
  );
}
