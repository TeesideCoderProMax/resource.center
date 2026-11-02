"use client";

import Link from "next/link";
import { useState } from "react";
import { call, fmt } from "@/lib/api-client";
import type { DeviceModelSummary } from "@/lib/loan";
import { errorNotice, Notice, type NoticeValue } from "./Notice";

/**
 * Reserve/subscribe card. Counts are never changed optimistically: after every attempt the
 * parent reloads from the API, which stays authoritative.
 */
export function DeviceCard({ item, canAct, detail = false, onChanged }: {
  item: DeviceModelSummary;
  canAct: boolean;
  detail?: boolean;
  onChanged?: () => void | Promise<void>;
}) {
  const [busy, setBusy] = useState(false);
  const [notice, setNotice] = useState<NoticeValue>(null);
  const Title = detail ? "h1" : "h3";
  const returnTo = `/devices/${encodeURIComponent(item.id)}`;
  // A 409 last-unit race means the shown count is stale; offer subscribe straight away.
  const subscribe = item.availableUnits <= 0 || notice?.hint === "subscribe";

  async function act(action: "reservations" | "subscriptions") {
    if (busy) return;
    setBusy(true);
    setNotice(null);
    try {
      if (action === "reservations") {
        const r = await call<{ dueAt: string; unitId: string }>("POST", action, { modelId: item.id });
        setNotice({
          kind: "ok",
          text: `Reserved ${item.name} (unit ${r.unitId}). Due back ${fmt(r.dueAt)}. Next: collect it at the Resource Center desk; staff will mark it collected.`,
          link: { href: "/reservations", label: "View my reservations" },
        });
      } else {
        await call("POST", action, { modelId: item.id });
        setNotice({ kind: "ok", text: `Subscribed. You will be emailed when ${item.name} is available.` });
      }
    } catch (e) {
      setNotice(errorNotice(e, returnTo));
    } finally {
      setBusy(false);
      await onChanged?.();
    }
  }

  return (
    <article className={detail ? "card detail" : "card"}>
      <div className="row">
        <span className="cat">{item.category}</span>
        <span className={`badge ${item.availableUnits > 0 ? "good" : "warn"}`}>
          {item.availableUnits} / {item.totalUnits} available
        </span>
      </div>
      <Title>{detail ? item.name : <Link href={returnTo}>{item.name}</Link>}</Title>
      <p className="muted">{item.description}</p>
      <p className="muted small">{item.loanDurationDays}-day loan · {item.id}</p>
      <Notice notice={notice} />
      {canAct && (
        subscribe ? (
          <button className="btn" disabled={busy} aria-busy={busy} onClick={() => act("subscriptions")}>
            {busy ? "Subscribing…" : "Notify me when available"}
          </button>
        ) : (
          <button className="btn primary" disabled={busy} aria-busy={busy} onClick={() => act("reservations")}>
            {busy ? "Reserving…" : "Reserve"}
          </button>
        )
      )}
    </article>
  );
}
