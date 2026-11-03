"use client";

import Link from "next/link";
import { useState } from "react";
import { call, fmt } from "@/lib/api-client";
import { useBag } from "@/lib/bag";
import type { DeviceModelSummary } from "@/lib/loan";
import { categoryIcon, Icon } from "./Icon";
import { errorNotice, Notice, type NoticeValue } from "./Notice";

export const devicePath = (id: string) => `/devices/${encodeURIComponent(id)}`;

/** Initials of the first two words, shown in the preview tile (the catalogue has no product photos). */
export const monogram = (name: string) => name.split(/\s+/).slice(0, 2).map((w) => w[0]?.toUpperCase() ?? "").join("");

/** Emerald when stocked, amber for the last unit, lilac when only the waitlist is open (PRD 5.1). */
export function AvailabilityPill({ item }: { item: DeviceModelSummary }) {
  const tone = item.availableUnits <= 0 ? "out" : item.availableUnits === 1 ? "low" : "in";
  const note = tone === "out" ? " · waitlist" : tone === "low" ? " · last one" : "";
  return (
    <span className={`pill pill-${tone}`}>
      <span className="dot" aria-hidden="true" />
      <span>{item.availableUnits} / {item.totalUnits} available</span>
      {note && <span>{note}</span>}
    </span>
  );
}

export function DevicePreview({ item, large = false }: { item: DeviceModelSummary; large?: boolean }) {
  return (
    <div className={large ? "showcase" : "preview"} aria-hidden="true">
      <span className="preview-icon"><Icon name={categoryIcon(item.category)} size={large ? 40 : 24} /></span>
      <span className="mono">{monogram(item.name)}</span>
    </div>
  );
}

export function AddToBag({ item, wide = false }: { item: DeviceModelSummary; wide?: boolean }) {
  const bag = useBag();
  if (!bag) return null;
  const inBag = bag.has(item.id);
  return (
    <button className={`btn${wide ? " wide" : ""}`} aria-pressed={inBag} onClick={() => (inBag ? bag.remove(item.id) : bag.add(item.id))}>
      <Icon name={inBag ? "check" : "bag"} />
      {inBag ? "In loan bag" : "Add to loan bag"}
    </button>
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
          <Icon name="calendar" />{busy ? "Reserving…" : "Reserve"}
        </button>
      )}
    </>
  );
}

export function DeviceCard({ item, canAct, onChanged, layout = "grid" }: {
  item: DeviceModelSummary;
  canAct: boolean;
  onChanged?: () => void | Promise<void>;
  layout?: "grid" | "list";
}) {
  return (
    <article className={`glass device-card ${layout}`}>
      <div className="preview-wrap">
        <DevicePreview item={item} />
        <div className="pills overlay">
          <AvailabilityPill item={item} />
          <span className="pill pill-plain">{item.category}</span>
        </div>
      </div>
      <div className="device-body">
        <p className="meta">
          <span><Icon name="clock" size={14} /> Max {item.loanDurationDays} days</span>
          <span className="accent"><Icon name="pin" size={14} /> Resource Center desk</span>
        </p>
        <h3><Link href={devicePath(item.id)}>{item.name}</Link></h3>
        <p className="muted">{item.description}</p>
        <span className="tag">{item.id}</span>
      </div>
      <div className="card-foot">
        <div className="small">
          <span className="muted block">Pickup method</span>
          <strong>Desk hand-off</strong>
        </div>
        {canAct && (
          <div className="card-actions">
            <AddToBag item={item} />
            <ReserveActions item={item} onChanged={onChanged} />
          </div>
        )}
      </div>
    </article>
  );
}
