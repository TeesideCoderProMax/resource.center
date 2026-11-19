"use client";

import Link from "next/link";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { call } from "@/lib/api-client";
import type { DeviceModelSummary } from "@/lib/loan";
import { DeviceCard } from "./DeviceCard";
import { categoryIcon, Icon } from "./Icon";
import { errorNotice, Notice, type NoticeValue } from "./Notice";

const PAGE_SIZE = 8;
// shortcut: the catalogue has no manufacturer field, so brand comes from the model name; add a brand column when one exists.
const MULTI_WORD_BRANDS = ["Texas Instruments", "Raspberry Pi"];
export const brandOf = (name: string) =>
  name.startsWith("iPad") ? "Apple" : MULTI_WORD_BRANDS.find((b) => name.startsWith(b)) ?? name.split(" ")[0];

const toggle = (list: string[], value: string) => list.includes(value) ? list.filter((v) => v !== value) : [...list, value];
const countBy = (values: string[]) => {
  const counts = new Map<string, number>();
  for (const v of values) counts.set(v, (counts.get(v) ?? 0) + 1);
  return [...counts].sort(([a], [b]) => a.localeCompare(b));
};

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
  const [cats, setCats] = useState<string[]>([]);
  const [brands, setBrands] = useState<string[]>([]);
  const [page, setPage] = useState(1);
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

  const categories = useMemo(() => countBy(items.map((i) => i.category)), [items]);
  const brandCounts = useMemo(() => countBy(items.map((i) => brandOf(i.name))), [items]);
  const durations = useMemo(() => [...new Set(items.map((i) => i.loanDurationDays))].sort((a, b) => a - b), [items]);

  const shown = useMemo(() => {
    const q = query.trim().toLowerCase();
    return items
      .filter((i) => cats.length === 0 || cats.includes(i.category))
      .filter((i) => brands.length === 0 || brands.includes(brandOf(i.name)))
      .filter((i) => !availableOnly || i.availableUnits > 0)
      .filter((i) => duration === null || i.loanDurationDays === duration)
      .filter((i) => !q || `${i.name} ${i.description} ${i.category} ${i.id}`.toLowerCase().includes(q))
      .sort(SORTS[sort]);
  }, [items, query, cats, brands, availableOnly, duration, sort]);

  // Any filter or sort change starts again from the first page.
  useEffect(() => { setPage(1); }, [query, cats, brands, availableOnly, duration, sort]);
  const pages = Math.max(1, Math.ceil(shown.length / PAGE_SIZE));
  const current = Math.min(page, pages);
  const resultsRef = useRef<HTMLElement>(null);
  const go = (n: number) => { setPage(n); resultsRef.current?.scrollIntoView?.({ block: "start" }); };
  const pageItems = shown.slice((current - 1) * PAGE_SIZE, current * PAGE_SIZE);

  const reset = () => { setQuery(""); setCats([]); setBrands([]); setAvailableOnly(false); setDuration(null); };

  return (
    <>
      <nav aria-label="Breadcrumb" className="crumbs">
        <Link href="/">Home</Link><span aria-hidden="true">›</span>
        <span>Equipment Catalog</span><span aria-hidden="true">›</span>
        <span aria-current="page" className="accent">{cats.length === 1 ? cats[0] : cats.length > 1 ? `${cats.length} categories` : "All Hardware"}</span>
      </nav>

      <h1 className="sr-only">Equipment catalog</h1>

      {categories.length > 1 && (
        <div className="chips-block">
          <div className="row"><strong>Categories</strong></div>
          <div className="chips" role="group" aria-label="Categories">
            <button className="chip" aria-pressed={cats.length === 0} onClick={() => setCats([])}>
              <Icon name="grid" /> All devices <span className="count">{items.length}</span>
            </button>
            {categories.map(([c, n]) => (
              <button key={c} className="chip" aria-pressed={cats.includes(c)} onClick={() => setCats(toggle(cats, c))}>
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
          {categories.length > 1 && (
            <fieldset className="filter-group">
              <legend className="field-label">Category</legend>
              {categories.map(([c, n]) => (
                <label key={c} className="check check-count">
                  <input type="checkbox" checked={cats.includes(c)} onChange={() => setCats(toggle(cats, c))} />
                  <span>{c}</span><span className="n">{n}</span>
                </label>
              ))}
            </fieldset>
          )}
          {brandCounts.length > 1 && (
            <fieldset className="filter-group">
              <legend className="field-label">Manufacturer / Brand</legend>
              <div className="check-grid">
                {brandCounts.map(([b]) => (
                  <label key={b} className="check">
                    <input type="checkbox" checked={brands.includes(b)} onChange={() => setBrands(toggle(brands, b))} /> {b}
                  </label>
                ))}
              </div>
            </fieldset>
          )}
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

        <section ref={resultsRef} aria-labelledby="catalogue-h" className="results">
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
            {pageItems.map((item) => <DeviceCard key={item.id} item={item} canAct={canAct} onChanged={refresh} layout={layout} />)}
          </div>
          {pages > 1 && (
            <nav className="pager" aria-label="Pages">
              <button className="btn" disabled={current === 1} onClick={() => go(current - 1)}>‹ Previous</button>
              {Array.from({ length: pages }, (_, i) => i + 1).map((n) => (
                <button key={n} className="btn" aria-current={n === current ? "page" : undefined} aria-label={`Page ${n}`} onClick={() => go(n)}>{n}</button>
              ))}
              <button className="btn" disabled={current === pages} onClick={() => go(current + 1)}>Next ›</button>
            </nav>
          )}
        </section>
      </div>
    </>
  );
}
