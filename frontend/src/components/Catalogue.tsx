"use client";

import { useCallback, useState } from "react";
import { call, describe } from "@/lib/api-client";
import type { DeviceModelSummary } from "@/lib/loan";
import { DeviceCard } from "./DeviceCard";
import { Notice, type NoticeValue } from "./Notice";

export function Catalogue({ initialItems, initialError, canAct }: { initialItems: DeviceModelSummary[]; initialError: string | null; canAct: boolean }) {
  const [items, setItems] = useState(initialItems);
  const [notice, setNotice] = useState<NoticeValue>(initialError ? { kind: "error", text: initialError } : null);

  const refresh = useCallback(async () => {
    try {
      setItems((await call<{ items: DeviceModelSummary[] }>("GET", "catalogue")).items);
    } catch (e) {
      setNotice({ kind: "error", text: describe(e) });
    }
  }, []);

  return (
    <section aria-labelledby="catalogue-h">
      <h2 id="catalogue-h">Devices</h2>
      <Notice notice={notice} />
      {items.length === 0 && !notice && <p className="muted">No devices in the catalogue.</p>}
      <div className="grid">
        {items.map((item) => <DeviceCard key={item.id} item={item} canAct={canAct} onChanged={refresh} />)}
      </div>
    </section>
  );
}
