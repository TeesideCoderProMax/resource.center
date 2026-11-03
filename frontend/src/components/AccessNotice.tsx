import type { Access } from "@/lib/roles";

/** Rendered by server pages when a display guard blocks a route. The Loan API still enforces RBAC. */
export function AccessNotice({ access, returnTo }: { access: Exclude<Access, "ok">; returnTo: string }) {
  switch (access) {
    case "auth-not-configured":
      return <section className="panel"><h1>Sign-in not configured</h1><p className="muted">This environment has no Auth0 settings, so signed-in pages are unavailable.</p></section>;
    case "sign-in":
      return (
        <section className="panel">
          <h1>Sign in required</h1>
          <p className="muted">Sign in with your campus account to continue.</p>
          <a className="btn primary" href={`/auth/login?returnTo=${encodeURIComponent(returnTo)}`}>Sign in</a>
        </section>
      );
    case "no-role":
      return <section className="panel"><h1>No role assigned</h1><p role="alert" className="alert">Your account has no student or staff role. Ask an administrator.</p></section>;
    case "forbidden":
      return <section className="panel"><h1>403 — staff only</h1><p role="alert" className="alert">This page is for desk staff. Your account does not have the staff role.</p></section>;
  }
}
