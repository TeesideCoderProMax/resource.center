import { auth0, auth0Configured } from "@/lib/auth0";
import { displayRoles, loanApiBaseUrl, type DeviceModelSummary } from "@/lib/loan";
import { Catalogue, MyReservations, StaffDesk } from "./ui";

export const dynamic = "force-dynamic";

async function fetchCatalogue(): Promise<{ items: DeviceModelSummary[]; error: string | null }> {
  const base = loanApiBaseUrl();
  if (!base) return { items: [], error: "LOAN_API_BASE_URL is not configured" };
  try {
    const res = await fetch(`${base}/api/v1/catalogue`, { cache: "no-store" });
    if (!res.ok) return { items: [], error: `Loan API returned HTTP ${res.status}` };
    return { items: ((await res.json()) as { items: DeviceModelSummary[] }).items, error: null };
  } catch {
    return { items: [], error: "Could not reach the Loan API" };
  }
}

export default async function HomePage() {
  const session = auth0 ? await auth0.getSession() : null;
  const roles = displayRoles(session?.tokenSet.accessToken);
  const isStaff = roles.includes("staff");
  const canBorrow = isStaff || roles.includes("student");
  const { items, error } = await fetchCatalogue();

  return (
    <main className="wrap">
      <header className="top">
        <div>
          <h1>Campus Device Loan</h1>
          <p className="muted">Reserve a device for a 2-day loan. Collect and return it at the desk.</p>
        </div>
        <nav>
          {!auth0Configured ? (
            <span className="badge warn">Sign-in not configured</span>
          ) : session ? (
            <>
              <span className="muted">
                {session.user.email ?? session.user.name} · {roles.join(", ") || "no role"}
              </span>
              <a className="btn" href="/auth/logout">Sign out</a>
            </>
          ) : (
            <a className="btn primary" href="/auth/login">Sign in</a>
          )}
        </nav>
      </header>

      {session && !canBorrow && (
        <p role="alert" className="alert">Your account has no student or staff role. Ask an administrator.</p>
      )}

      <Catalogue initialItems={items} initialError={error} signedIn={Boolean(session) && canBorrow} />
      {session && canBorrow && !isStaff && <MyReservations title="My reservations" />}
      {isStaff && <StaffDesk />}
    </main>
  );
}
