import Link from "next/link";
import { canBorrow, navLinks, roleLabel, type Viewer } from "@/lib/roles";
import { BagButton, HeaderSearch, MainNav } from "./HeaderParts";
import { Icon } from "./Icon";

const initials = (email: string | null) => (email ?? "?").split("@")[0].split(/[._-]/).slice(0, 2).map((p) => p[0]?.toUpperCase() ?? "").join("") || "?";

export function AppHeader({ viewer }: { viewer: Viewer }) {
  return (
    <header className="top">
      <div className="top-inner">
        <div className="top-left">
          <Link href="/" className="brand"><span className="brand-mark" aria-hidden="true"><Icon name="laptop" size={20} /></span>Campus Device Hub</Link>
          <HeaderSearch />
        </div>
        <MainNav links={navLinks(viewer)} />
        <div className="account">
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
