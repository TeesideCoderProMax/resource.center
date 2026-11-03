import { Catalogue } from "@/components/Catalogue";
import { canBorrow } from "@/lib/roles";
import { fetchCatalogue, getViewer } from "@/lib/viewer";

export const dynamic = "force-dynamic";

export default async function HomePage({ searchParams }: { searchParams: Promise<{ q?: string | string[] }> }) {
  const [viewer, { items, error }, { q }] = await Promise.all([getViewer(), fetchCatalogue(), searchParams]);
  const canAct = viewer.signedIn && canBorrow(viewer);
  return (
    <>
      {viewer.signedIn && !canBorrow(viewer) && (
        <p role="alert" className="alert">Your account has no student or staff role. Ask an administrator.</p>
      )}
      {!viewer.signedIn && viewer.authConfigured && <p className="muted">Sign in to reserve a device.</p>}
      <Catalogue initialItems={items} initialError={error} canAct={canAct} initialQuery={typeof q === "string" ? q : ""} />
    </>
  );
}
