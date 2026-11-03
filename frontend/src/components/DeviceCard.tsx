"use client";

import Link from "next/link";
import { useState } from "react";
import { call, fmt } from "@/lib/api-client";
import type { DeviceModelSummary } from "@/lib/loan";
import { errorNotice, Notice, type NoticeValue } from "./Notice";

export const devicePath = (id: string) => `/devices/${encodeURIComponent(id)}`;

/** Initials of the first two words, shown in the preview tile (the catalogue has no product photos). */
export const monogram = (name: string) => name.split(/\s+/).slice(0, 2).map((w) => w[0]?.toUpperCase() ?? "").join("");

export function AvailabilityPill({ item }: { item: DeviceModelSummary }) {
  const tone = item.availableUnits <= 0 ? "out" : item.availableUnits === 1 ? "low" : "in";
  return (
    <span className={`pill pill-${tone}`}>
      <span className="dot" aria-hidden="true" />
      {item.availableUnits} / {item.totalUnits} available
    </span>
  );
}

/**
 * Reserve/subscribe actions. Counts are never changed optimistically: after every attempt the
 * parent reloads from the API, which stays authoritative.
 */
export function ReserveActions({ item, onChanged, wide = false }: {
  item: DeviceModelSummary;
  onChanged?: () => void | Promise<void>;
  wide?: boolean;
}) {
  const [busy, setBusy] = useState(false);
  const [notice, setNotice] = useState<NoticeValue>(null);
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
      setNotice(errorNotice(e, devicePath(item.id)));
    } finally {
      setBusy(false);
      await onChanged?.();
    }
  }

  const cls = wide ? " wide" : "";
  return (
    <>
      <Notice notice={notice} />
      {subscribe ? (
        <button className={`btn${cls}`} disabled={busy} aria-busy={busy} onClick={() => act("subscriptions")}>
          {busy ? "Subscribing…" : "Notify me when available"}
        </button>
      ) : (
        <button className={`btn primary${cls}`} disabled={busy} aria-busy={busy} onClick={() => act("reservations")}>
          {busy ? "Reserving…" : "Reserve"}
        </button>
      )}
    </>
  );
}

export function DeviceCard({ item, canAct, onChanged }: {
  item: DeviceModelSummary;
  canAct: boolean;
  onChanged?: () => void | Promise<void>;
}) {
  return (
    <article className="glass device-card">
      <div className="preview" aria-hidden="true"><span className="mono">{monogram(item.name)}</span></div>
      <div className="pills">
        <AvailabilityPill item={item} />
        <span className="pill pill-plain">{item.category}</span>
      </div>
      <p className="meta">{item.loanDurationDays}-day loan · collect at the desk</p>
      <h3><Link href={devicePath(item.id)}>{item.name}</Link></h3>
      <p className="muted">{item.description}</p>
      <div className="card-foot">
        <span className="muted small">{item.id}</span>
        {canAct && <div className="card-actions"><ReserveActions item={item} onChanged={onChanged} /></div>}
      </div>
    </article>
  );
}
