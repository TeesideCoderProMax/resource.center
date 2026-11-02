import { Catalogue } from "@/components/Catalogue";
import { canBorrow } from "@/lib/roles";
import { fetchCatalogue, getViewer } from "@/lib/viewer";

export const dynamic = "force-dynamic";

export default async function HomePage() {
  const [viewer, { items, error }] = await Promise.all([getViewer(), fetchCatalogue()]);
  const canAct = viewer.signedIn && canBorrow(viewer);
  return (
    <>
      <p className="lede muted">Reserve a device for a 2-day loan. Collect and return it at the desk.</p>
      {viewer.signedIn && !canBorrow(viewer) && (
        <p role="alert" className="alert">Your account has no student or staff role. Ask an administrator.</p>
      )}
      <Catalogue initialItems={items} initialError={error} canAct={canAct} />
      {!viewer.signedIn && viewer.authConfigured && <p className="muted">Sign in to reserve a device.</p>}
    </>
  );
}
