import Link from "next/link";
import { canBorrow, navLinks, roleLabel, type Viewer } from "@/lib/roles";
import { BagButton, MainNav } from "./HeaderParts";
import { BrandMark } from "./BrandMark";
import { ThemeToggle } from "./ThemeToggle";

const initials = (email: string | null) => (email ?? "?").split("@")[0].split(/[._-]/).slice(0, 2).map((p) => p[0]?.toUpperCase() ?? "").join("") || "?";

export function AppHeader({ viewer }: { viewer: Viewer }) {
  return (
    <header className="top">
      <div className="top-inner">
        <div className="top-left">
          <Link href="/" className="brand">
            <BrandMark />
            <span className="brand-wordmark">
              <span className="brand-name">Campus Device</span>{" "}
              <span className="brand-accent">Hub</span>
            </span>
          </Link>
        </div>
        <MainNav links={navLinks(viewer)} />
        <div className="account">
          <ThemeToggle />
          {!viewer.authConfigured ? (
            <span className="badge warn">Sign-in not configured</span>
          ) : viewer.signedIn ? (
            <>
              {canBorrow(viewer) && <BagButton />}
              <span className="badge role">{roleLabel(viewer)}</span>
              <a className="btn" href="/auth/logout">Sign out</a>
              <span className="avatar" title={viewer.email ?? undefined} aria-label={`Signed in as ${viewer.email ?? "unknown"}`}>{initials(viewer.email)}</span>
            </>
          ) : (
            <a className="btn primary" href="/auth/login">Sign in</a>
          )}
        </div>
      </div>
    </header>
  );
}
