"use client";

import { useCallback, useMemo, useState } from "react";
import { call } from "@/lib/api-client";
import type { DeviceModelSummary } from "@/lib/loan";
import { DeviceCard } from "./DeviceCard";
import { errorNotice, Notice, type NoticeValue } from "./Notice";

type Sort = "availability" | "name";

export function Catalogue({ initialItems, initialError, canAct }: { initialItems: DeviceModelSummary[]; initialError: string | null; canAct: boolean }) {
  const [items, setItems] = useState(initialItems);
  const [notice, setNotice] = useState<NoticeValue>(initialError ? { kind: "error", text: initialError, hint: "degraded" } : null);
  const [query, setQuery] = useState("");
  const [category, setCategory] = useState<string | null>(null);
  const [availableOnly, setAvailableOnly] = useState(false);
  const [sort, setSort] = useState<Sort>("availability");

  const refresh = useCallback(async () => {
    try {
      setItems((await call<{ items: DeviceModelSummary[] }>("GET", "catalogue")).items);
      setNotice(null);
    } catch (e) {
      setNotice(errorNotice(e, "/"));
    }
  }, []);

  const categories = useMemo(() => {
    const counts = new Map<string, number>();
    for (const i of items) counts.set(i.category, (counts.get(i.category) ?? 0) + 1);
    return [...counts].sort(([a], [b]) => a.localeCompare(b));
  }, [items]);

  const shown = useMemo(() => {
    const q = query.trim().toLowerCase();
    return items
      .filter((i) => !category || i.category === category)
      .filter((i) => !availableOnly || i.availableUnits > 0)
      .filter((i) => !q || `${i.name} ${i.description} ${i.category}`.toLowerCase().includes(q))
      .sort((a, b) => sort === "name" ? a.name.localeCompare(b.name) : b.availableUnits - a.availableUnits || a.name.localeCompare(b.name));
  }, [items, query, category, availableOnly, sort]);

  const totalUnits = items.reduce((n, i) => n + i.totalUnits, 0);
  const availableUnits = items.reduce((n, i) => n + i.availableUnits, 0);
  const reset = () => { setQuery(""); setCategory(null); setAvailableOnly(false); };

  return (
    <>
      <div className="hero">
        <div>
          <span className="eyebrow">Campus device loans</span>
          <h1>Equipment catalogue</h1>
          <p className="muted">Reserve a device, collect it at the Resource Center desk and return it before it is due.</p>
        </div>
        <dl className="stats">
          <div><dt>Device models</dt><dd>{items.length}</dd></div>
          <div><dt>Units available</dt><dd className="good-text">{availableUnits}</dd></div>
          <div><dt>Units in total</dt><dd>{totalUnits}</dd></div>
        </dl>
      </div>

      {categories.length > 1 && (
        <div className="chips" role="group" aria-label="Categories">
          <button className="chip" aria-pressed={category === null} onClick={() => setCategory(null)}>
            All devices <span className="count">{items.length}</span>
          </button>
          {categories.map(([c, n]) => (
            <button key={c} className="chip" aria-pressed={category === c} onClick={() => setCategory(c)}>
              {c} <span className="count">{n}</span>
            </button>
          ))}
        </div>
      )}

      <div className="catalogue-layout">
        <aside className="glass filters-panel" aria-label="Filters">
          <div className="row">
            <strong>Filters</strong>
            <button className="link-btn" onClick={reset}>Reset all</button>
          </div>
          <label className="field-label" htmlFor="catalogue-search">Search</label>
          <input id="catalogue-search" type="search" placeholder="Name, category, description…" value={query} onChange={(e) => setQuery(e.target.value)} />
          <label className="check">
            <input type="checkbox" checked={availableOnly} onChange={(e) => setAvailableOnly(e.target.checked)} />
            Available now only
          </label>
          <div className="tip">
            <strong>Borrower note</strong>
            <p>If a device is fully reserved, subscribe and you will be emailed when a unit is returned.</p>
          </div>
        </aside>

        <section aria-labelledby="catalogue-h" className="results">
          <div className="glass results-bar">
            <div className="row-start">
              <h2 id="catalogue-h">Devices</h2>
              <span className="pill pill-accent">{shown.length} of {items.length} models</span>
            </div>
            <label className="sort">
              <span className="muted small">Sort by</span>
              <select value={sort} onChange={(e) => setSort(e.target.value as Sort)}>
                <option value="availability">Highest availability</option>
                <option value="name">Name (A–Z)</option>
              </select>
            </label>
          </div>
          <Notice notice={notice} />
          {items.length === 0 && !notice && <p className="muted">No devices in the catalogue.</p>}
          {items.length > 0 && shown.length === 0 && <p className="muted">No devices match these filters.</p>}
          <div className="grid">
            {shown.map((item) => <DeviceCard key={item.id} item={item} canAct={canAct} onChanged={refresh} />)}
          </div>
        </section>
      </div>
    </>
  );
}
