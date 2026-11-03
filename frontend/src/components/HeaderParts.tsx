"use client";

import Link from "next/link";
import { usePathname, useRouter } from "next/navigation";
import { useEffect, useRef } from "react";
import { useBag } from "@/lib/bag";
import { Icon } from "./Icon";

export function MainNav({ links }: { links: { href: string; label: string }[] }) {
  const path = usePathname();
  const active = (href: string) => (href === "/" ? path === "/" || path.startsWith("/devices") : path.startsWith(href));
  return (
    <nav aria-label="Main">
      <ul className="nav">
        {links.map((l) => (
          <li key={l.href}><Link href={l.href} aria-current={active(l.href) ? "page" : undefined}>{l.label}</Link></li>
        ))}
      </ul>
    </nav>
  );
}

/** Catalogue search from any page; Ctrl/⌘+K focuses it. */
export function HeaderSearch() {
  const router = useRouter();
  const ref = useRef<HTMLInputElement>(null);
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === "k") { e.preventDefault(); ref.current?.focus(); }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);
  return (
    <form role="search" className="top-search" onSubmit={(e) => {
      e.preventDefault();
      const q = ref.current?.value.trim() ?? "";
      router.push(q ? `/?q=${encodeURIComponent(q)}` : "/");
    }}>
      <Icon name="search" />
      <input ref={ref} type="search" aria-label="Search equipment" placeholder="Search equipment…" />
      <kbd aria-hidden="true">⌘K</kbd>
    </form>
  );
}

export function BagButton() {
  const ids = useBag()?.ids ?? [];
  return (
    <Link href="/bag" className="icon-btn" aria-label={`Loan bag, ${ids.length} item${ids.length === 1 ? "" : "s"}`}>
      <Icon name="bag" size={20} />
      {ids.length > 0 && <span className="count-badge" aria-hidden="true">{ids.length}</span>}
    </Link>
  );
}
