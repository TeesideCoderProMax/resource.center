"use client";

import { createContext, useCallback, useContext, useEffect, useMemo, useState } from "react";

/**
 * Client-side loan bag: model IDs only, kept in this browser. It is a convenience list, not a
 * reservation; the Loan API reserves each model separately when the borrower submits.
 */
interface Bag {
  ids: string[];
  has: (id: string) => boolean;
  add: (id: string) => void;
  remove: (id: string) => void;
  clear: () => void;
}

const KEY = "loan-bag:v1";
const BagContext = createContext<Bag | null>(null);

function read(): string[] {
  try {
    const v = JSON.parse(localStorage.getItem(KEY) ?? "[]");
    return Array.isArray(v) ? v.filter((x): x is string => typeof x === "string") : [];
  } catch {
    return [];
  }
}

export function BagProvider({ children }: { children: React.ReactNode }) {
  const [ids, setIds] = useState<string[]>([]);

  useEffect(() => { setIds(read()); }, []);
  useEffect(() => {
    const sync = (e: StorageEvent) => { if (e.key === KEY) setIds(read()); };
    window.addEventListener("storage", sync);
    return () => window.removeEventListener("storage", sync);
  }, []);

  const save = useCallback((update: (prev: string[]) => string[]) => {
    setIds((prev) => {
      const next = update(prev);
      try { localStorage.setItem(KEY, JSON.stringify(next)); } catch { /* storage blocked: keep in memory */ }
      return next;
    });
  }, []);

  const value = useMemo<Bag>(() => ({
    ids,
    has: (id) => ids.includes(id),
    add: (id) => save((p) => (p.includes(id) ? p : [...p, id])),
    remove: (id) => save((p) => p.filter((x) => x !== id)),
    clear: () => save(() => []),
  }), [ids, save]);

  return <BagContext.Provider value={value}>{children}</BagContext.Provider>;
}

/** Null outside BagProvider (e.g. isolated component tests): bag controls then render nothing. */
export function useBag(): Bag | null {
  return useContext(BagContext);
}
