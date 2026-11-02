"use client";

import Link from "next/link";
import { useState } from "react";
import { call, describe, fmt } from "@/lib/api-client";
import type { DeviceModelSummary } from "@/lib/loan";
import { Notice, type NoticeValue } from "./Notice";

export function DeviceCard({ item, canAct, detail = false, onChanged }: {
  item: DeviceModelSummary;
  canAct: boolean;
  detail?: boolean;
  onChanged?: () => void | Promise<void>;
}) {
  const [busy, setBusy] = useState(false);
  const [notice, setNotice] = useState<NoticeValue>(null);
  const Title = detail ? "h1" : "h3";

  async function act(action: "reservations" | "subscriptions") {
    setBusy(true);
    setNotice(null);
    try {
      if (action === "reservations") {
        const r = await call<{ dueAt: string; unitId: string }>("POST", action, { modelId: item.id });
        setNotice({ kind: "ok", text: `Reserved ${item.name} (unit ${r.unitId}). Return by ${fmt(r.dueAt)}.` });
      } else {
        await call("POST", action, { modelId: item.id });
        setNotice({ kind: "ok", text: `Subscribed. We will email you when ${item.name} is back.` });
      }
    } catch (e) {
      setNotice({ kind: "error", text: describe(e) });
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
      <Title>{detail ? item.name : <Link href={`/devices/${encodeURIComponent(item.id)}`}>{item.name}</Link>}</Title>
      <p className="muted">{item.description}</p>
      <p className="muted small">{item.loanDurationDays}-day loan · {item.id}</p>
      <Notice notice={notice} />
      {canAct && (
        item.availableUnits > 0 ? (
          <button className="btn primary" disabled={busy} onClick={() => act("reservations")}>
            {busy ? "Reserving…" : "Reserve"}
          </button>
        ) : (
          <button className="btn" disabled={busy} onClick={() => act("subscriptions")}>
            {busy ? "Subscribing…" : "Notify me when available"}
          </button>
        )
      )}
    </article>
  );
}
