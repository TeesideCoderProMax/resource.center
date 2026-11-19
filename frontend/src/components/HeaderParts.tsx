"use client";

import Link from "next/link";
import { usePathname } from "next/navigation";
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

export function BagButton() {
  const ids = useBag()?.ids ?? [];
  return (
    <Link href="/bag" className="icon-btn" aria-label={`Loan bag, ${ids.length} item${ids.length === 1 ? "" : "s"}`}>
      <Icon name="bag" size={20} />
      {ids.length > 0 && <span className="count-badge" aria-hidden="true">{ids.length}</span>}
    </Link>
  );
}
