import Link from "next/link";
import { navLinks, roleLabel, type Viewer } from "@/lib/roles";

export function AppHeader({ viewer }: { viewer: Viewer }) {
  return (
    <header className="top">
      <Link href="/" className="brand">Campus Device Loan</Link>
      <nav aria-label="Main">
        <ul className="nav">
          {navLinks(viewer).map((l) => <li key={l.href}><Link href={l.href}>{l.label}</Link></li>)}
        </ul>
      </nav>
      <div className="account">
        {!viewer.authConfigured ? (
          <span className="badge warn">Sign-in not configured</span>
        ) : viewer.signedIn ? (
          <>
            <span className="muted small">{viewer.email}</span>
            <span className="badge role">{roleLabel(viewer)}</span>
            <a className="btn" href="/auth/logout">Sign out</a>
          </>
        ) : (
          <a className="btn primary" href="/auth/login">Sign in</a>
        )}
      </div>
    </header>
  );
}
