"use client";

import Link from "next/link";
import { useCallback, useEffect, useMemo, useState } from "react";
import { call } from "@/lib/api-client";
import type { DeviceModelSummary } from "@/lib/loan";
import { DeviceCard } from "./DeviceCard";
import { categoryIcon, Icon } from "./Icon";
import { errorNotice, Notice, type NoticeValue } from "./Notice";

type Sort = "availability" | "name" | "duration";

const SORTS: Record<Sort, (a: DeviceModelSummary, b: DeviceModelSummary) => number> = {
  availability: (a, b) => b.availableUnits - a.availableUnits || a.name.localeCompare(b.name),
  name: (a, b) => a.name.localeCompare(b.name),
  duration: (a, b) => a.loanDurationDays - b.loanDurationDays || a.name.localeCompare(b.name),
};

export function Catalogue({ initialItems, initialError, canAct, initialQuery = "" }: {
  initialItems: DeviceModelSummary[];
  initialError: string | null;
  canAct: boolean;
  initialQuery?: string;
}) {
  const [items, setItems] = useState(initialItems);
  const [notice, setNotice] = useState<NoticeValue>(initialError ? { kind: "error", text: initialError, hint: "degraded" } : null);
  const [query, setQuery] = useState(initialQuery);
  const [category, setCategory] = useState<string | null>(null);
  const [availableOnly, setAvailableOnly] = useState(false);
  const [duration, setDuration] = useState<number | null>(null);
  const [sort, setSort] = useState<Sort>("availability");
  const [layout, setLayout] = useState<"grid" | "list">("grid");

  // The header search navigates to /?q=…; follow it when already on this page.
  useEffect(() => { setQuery(initialQuery); }, [initialQuery]);

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
  const durations = useMemo(() => [...new Set(items.map((i) => i.loanDurationDays))].sort((a, b) => a - b), [items]);

  const shown = useMemo(() => {
    const q = query.trim().toLowerCase();
    return items
      .filter((i) => !category || i.category === category)
      .filter((i) => !availableOnly || i.availableUnits > 0)
      .filter((i) => duration === null || i.loanDurationDays === duration)
      .filter((i) => !q || `${i.name} ${i.description} ${i.category} ${i.id}`.toLowerCase().includes(q))
      .sort(SORTS[sort]);
  }, [items, query, category, availableOnly, duration, sort]);

  const reset = () => { setQuery(""); setCategory(null); setAvailableOnly(false); setDuration(null); };

  return (
    <>
      <nav aria-label="Breadcrumb" className="crumbs">
        <Link href="/">Home</Link><span aria-hidden="true">›</span>
        <span>Equipment Catalog</span><span aria-hidden="true">›</span>
        <span aria-current="page" className="accent">{category ?? "All Hardware"}</span>
      </nav>

      <h1 className="sr-only">Equipment catalog</h1>

      {categories.length > 1 && (
        <div className="chips-block">
          <div className="row"><strong>Categories</strong></div>
          <div className="chips" role="group" aria-label="Categories">
            <button className="chip" aria-pressed={category === null} onClick={() => setCategory(null)}>
              <Icon name="grid" /> All devices <span className="count">{items.length}</span>
            </button>
            {categories.map(([c, n]) => (
              <button key={c} className="chip" aria-pressed={category === c} onClick={() => setCategory(category === c ? null : c)}>
                <Icon name={categoryIcon(c)} /> {c} <span className="count">{n}</span>
              </button>
            ))}
          </div>
        </div>
      )}

      <div className="catalogue-layout">
        <aside className="glass filters-panel" aria-label="Filters">
          <div className="row filters-head">
            <strong className="h-sm"><Icon name="list" /> Filters</strong>
            <button className="link-btn" onClick={reset}>Reset all</button>
          </div>
          <div className="filter-group">
            <label className="field-label" htmlFor="catalogue-search">Search</label>
            <input id="catalogue-search" type="search" placeholder="Name, category, description…" value={query} onChange={(e) => setQuery(e.target.value)} />
          </div>
          <fieldset className="filter-group">
            <legend className="field-label">Availability</legend>
            <label className="check">
              <input type="checkbox" checked={availableOnly} onChange={(e) => setAvailableOnly(e.target.checked)} />
              Available now only
            </label>
          </fieldset>
          {durations.length > 0 && (
            <fieldset className="filter-group">
              <legend className="field-label">Maximum loan</legend>
              <label className="check">
                <input type="radio" name="duration" checked={duration === null} onChange={() => setDuration(null)} /> Any length
              </label>
              {durations.map((d) => (
                <label key={d} className="check">
                  <input type="radio" name="duration" checked={duration === d} onChange={() => setDuration(d)} /> {d} days
                </label>
              ))}
            </fieldset>
          )}
          <div className="tip">
            <strong><Icon name="info" size={14} /> Borrower note</strong>
            <p>Fully reserved? Subscribe and you are emailed when a unit is returned. Late returns are flagged to desk staff.</p>
          </div>
        </aside>

        <section aria-labelledby="catalogue-h" className="results">
          <div className="glass results-bar">
            <div className="row-start">
              <h2 className="sr-only" id="catalogue-h">Devices</h2>
              <span className="h-sm" aria-hidden="true">Available Hardware</span>
              <span className="pill pill-accent">{shown.length} of {items.length} results</span>
            </div>
            <div className="row-start">
              <label className="sort">
                <span className="muted small">Sort by</span>
                <select value={sort} onChange={(e) => setSort(e.target.value as Sort)}>
                  <option value="availability">Highest availability</option>
                  <option value="name">Name (A–Z)</option>
                  <option value="duration">Maximum loan (short to long)</option>
                </select>
              </label>
              <div className="segmented" role="group" aria-label="Layout">
                <button aria-pressed={layout === "grid"} onClick={() => setLayout("grid")} title="Grid view"><Icon name="grid" /><span className="sr-only">Grid view</span></button>
                <button aria-pressed={layout === "list"} onClick={() => setLayout("list")} title="List view"><Icon name="list" /><span className="sr-only">List view</span></button>
              </div>
            </div>
          </div>
          <Notice notice={notice} />
          {items.length === 0 && !notice && <p className="muted">No devices in the catalogue.</p>}
          {items.length > 0 && shown.length === 0 && <p className="muted">No devices match these filters.</p>}
          <div className={layout === "grid" ? "device-grid" : "device-list"}>
            {shown.map((item) => <DeviceCard key={item.id} item={item} canAct={canAct} onChanged={refresh} layout={layout} />)}
          </div>
        </section>
      </div>
    </>
  );
}
