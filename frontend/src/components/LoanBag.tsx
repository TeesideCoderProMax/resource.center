"use client";

import Link from "next/link";
import { useRouter } from "next/navigation";
import { useState } from "react";
import { call, fmt } from "@/lib/api-client";
import { useBag } from "@/lib/bag";
import type { DeviceModelSummary } from "@/lib/loan";
import { AvailabilityPill, DevicePreview, devicePath, LoanPeriod } from "./DeviceCard";
import { Icon } from "./Icon";
import { errorNotice, Notice, type NoticeValue } from "./Notice";

const estDue = (days: number) => new Date(Date.now() + days * 86_400_000).toLocaleDateString("en-GB", { day: "numeric", month: "short", year: "numeric" });

/**
 * Review page for the client-side bag. "Reserve all" sends one reservation per model, in order,
 * and reports each outcome: the Loan API has no multi-model transaction, so some may succeed
 * while others fail.
 */
export function LoanBag({ catalogue, catalogueError, email, role }: {
  catalogue: DeviceModelSummary[];
  catalogueError: string | null;
  email: string | null;
  role: string;
}) {
  const bag = useBag();
  const router = useRouter();
  const [ackReturn, setAckReturn] = useState(false);
  const [ackCare, setAckCare] = useState(false);
  const [periods, setPeriods] = useState<Record<string, number>>({});
  const [busy, setBusy] = useState(false);
  const [results, setResults] = useState<Record<string, NoticeValue>>({});
  const [reserved, setReserved] = useState<DeviceModelSummary[]>([]);

  const ids = bag?.ids ?? [];
  const items = ids.map((id) => catalogue.find((m) => m.id === id)).filter((m): m is DeviceModelSummary => Boolean(m));
  const missing = catalogueError ? [] : ids.filter((id) => !catalogue.some((m) => m.id === id));
  const latestDue = items.length ? estDue(Math.max(...items.map((i) => periods[i.id] ?? 1))) : "—";
  const canSubmit = items.length > 0 && ackReturn && ackCare && !busy;

  async function reserveAll() {
    if (!bag || !canSubmit) return;
    setBusy(true);
    setResults({});
    const done: DeviceModelSummary[] = [];
    for (const item of items) {
      try {
        const r = await call<{ dueAt: string; unitId: string }>("POST", "reservations", { modelId: item.id, loanDurationDays: periods[item.id] ?? 1 });
        setResults((p) => ({ ...p, [item.id]: { kind: "ok", text: `Reserved ${item.name} (unit ${r.unitId}). Due back ${fmt(r.dueAt)}.` } }));
        done.push(item);
        bag.remove(item.id);
      } catch (e) {
        setResults((p) => ({ ...p, [item.id]: errorNotice(e, "/bag") }));
      }
    }
    setReserved((p) => [...p, ...done]);
    setBusy(false);
    router.refresh();
  }

  async function subscribe(item: DeviceModelSummary) {
    try {
      await call("POST", "subscriptions", { modelId: item.id });
      setResults((p) => ({ ...p, [item.id]: { kind: "ok", text: `Subscribed. You will be emailed when ${item.name} is available.` } }));
      setReserved((p) => [...p, item]);
      bag?.remove(item.id);
    } catch (e) {
      setResults((p) => ({ ...p, [item.id]: errorNotice(e, "/bag") }));
    }
  }

  const step = reserved.length > 0 && items.length === 0 ? 3 : 2;

  return (
    <>
      <div className="page-head">
        <div>
          <span className="eyebrow"><Icon name="bag" size={14} /> Loan bag · instant reservations</span>
          <h1>Checkout &amp; Reservation Review</h1>
          <p className="muted">Review the devices in your bag, confirm the loan terms and reserve them in one go.</p>
        </div>
        <ol className="steps" aria-label="Progress">
          <li className="done"><span>{step > 1 ? <Icon name="check" size={14} /> : 1}</span> Select items</li>
          <li className={step > 2 ? "done" : "current"} aria-current={step === 2 ? "step" : undefined}><span>{step > 2 ? <Icon name="check" size={14} /> : 2}</span> Review terms</li>
          <li className={step === 3 ? "current" : ""} aria-current={step === 3 ? "step" : undefined}><span>3</span> Reserved</li>
        </ol>
      </div>

      <p className="notice-strip"><Icon name="info" /> <span><strong>Desk notice:</strong> reservations take effect immediately. Collect each device at the Resource Center desk; staff record the hand-over.</span></p>
      <Notice notice={catalogueError ? { kind: "error", text: catalogueError, hint: "degraded" } : null} />

      <div className="bag-layout">
        <section aria-labelledby="bag-items-h" className="bag-items">
          <div className="row">
            <div className="row-start">
              <h2 id="bag-items-h">Items in loan request</h2>
              <span className="pill pill-accent">{items.length} device{items.length === 1 ? "" : "s"}</span>
            </div>
            <Link href="/" className="link-btn"><Icon name="plus" size={14} /> Browse more equipment</Link>
          </div>

          {missing.length > 0 && (
            <p className="muted small">
              {missing.length} item{missing.length === 1 ? " is" : "s are"} no longer in the catalogue.{" "}
              <button className="link-btn" onClick={() => missing.forEach((id) => bag?.remove(id))}>Remove</button>
            </p>
          )}
          {items.length === 0 && reserved.length === 0 && (
            <div className="glass panel-lg empty">
              <p>Your loan bag is empty.</p>
              <Link href="/" className="btn primary">Browse the catalogue</Link>
            </div>
          )}

          {items.map((item) => (
            <article key={item.id} className="glass bag-item">
              <DevicePreview item={item} />
              <div className="bag-item-body">
                <div className="row row-wrap">
                  <h3><Link href={devicePath(item.id)}>{item.name}</Link></h3>
                  <AvailabilityPill item={item} />
                </div>
                <span className="tag">MODEL {item.id}</span>
                <LoanPeriod item={item} days={periods[item.id] ?? 1} onChange={(days) => setPeriods((p) => ({ ...p, [item.id]: days }))} disabled={busy} />
                <div className="bag-facts">
                  <div><Icon name="calendar" size={16} /><span><span className="muted block small">Chosen loan: {periods[item.id] ?? 1} {(periods[item.id] ?? 1) === 1 ? "day" : "days"}</span>Due about {estDue(periods[item.id] ?? 1)}</span></div>
                  <div><Icon name="pin" size={16} /><span><span className="muted block small">Pickup</span>Resource Center desk</span></div>
                </div>
                <Notice notice={results[item.id] ?? null} />
                <div className="row">
                  {results[item.id]?.hint === "subscribe" || item.availableUnits <= 0
                    ? <button className="link-btn" onClick={() => subscribe(item)}>Notify me when available instead</button>
                    : <span />}
                  <button className="link-btn danger" disabled={busy} onClick={() => bag?.remove(item.id)}><Icon name="trash" size={14} /> Remove item</button>
                </div>
              </div>
            </article>
          ))}

          {reserved.length > 0 && (
            <div className="glass panel-lg" role="region" aria-label="Done">
              <h2 className="h-sm"><Icon name="check" /> Done</h2>
              {reserved.map((item) => <Notice key={item.id} notice={results[item.id] ?? null} />)}
              <p className="muted small">Next: collect at the Resource Center desk. <Link href="/reservations">View my loans</Link></p>
            </div>
          )}
        </section>

        <aside className="glass panel-lg bag-summary" aria-labelledby="borrower-h">
          <div className="borrower">
            <div className="row">
              <span id="borrower-h" className="eyebrow">Borrower</span>
              <span className="pill pill-in">Signed in</span>
            </div>
            <div className="row-start">
              <span className="avatar lg" aria-hidden="true"><Icon name="user" /></span>
              <div>
                <strong className="block">{email ?? "Unknown account"}</strong>
                <span className="muted small">Campus account · {role}</span>
              </div>
            </div>
          </div>

          <dl className="summary">
            <div><dt>Devices</dt><dd>{items.length}</dd></div>
            <div><dt>Deposit</dt><dd className="good-text">None</dd></div>
            <div><dt>Pickup</dt><dd>Resource Center desk</dd></div>
            <div className="total"><dt>Latest estimated due date</dt><dd className="accent">{latestDue}</dd></div>
          </dl>

          <fieldset className="acks">
            <legend className="eyebrow">Loan terms</legend>
            <label className="check">
              <input type="checkbox" checked={ackReturn} onChange={(e) => setAckReturn(e.target.checked)} />
              I will return every device in working condition by its due time.
            </label>
            <label className="check">
              <input type="checkbox" checked={ackCare} onChange={(e) => setAckCare(e.target.checked)} />
              I am responsible for damage or loss while a device is on loan to me.
            </label>
          </fieldset>

          <button className="btn primary wide" disabled={!canSubmit} aria-busy={busy} onClick={reserveAll}>
            <Icon name="check" /> {busy ? "Reserving…" : `Reserve all (${items.length})`}
          </button>
          <button className="btn wide" disabled={busy || items.length === 0} onClick={() => bag?.clear()}>Clear bag</button>
          <p className="muted small center"><Icon name="shield" size={14} /> Signed in with your campus Auth0 account.</p>
        </aside>
      </div>
    </>
  );
}
