import { notFound } from "next/navigation";
import { DeviceDetail } from "./DeviceDetail";
import { canBorrow } from "@/lib/roles";
import { fetchCatalogue, getViewer } from "@/lib/viewer";

export const dynamic = "force-dynamic";

/** No single-model route exists in the contract, so the detail reads the public catalogue. */
export default async function DevicePage({ params }: { params: Promise<{ id: string }> }) {
  const { id } = await params;
  const [viewer, { items, error }] = await Promise.all([getViewer(), fetchCatalogue()]);
  if (error) return <p role="alert" className="alert">{error}</p>;
  const item = items.find((i) => i.id === id);
  if (!item) notFound();
  const canAct = viewer.signedIn && canBorrow(viewer);
  const signInHref = !viewer.signedIn && viewer.authConfigured ? `/auth/login?returnTo=${encodeURIComponent(`/devices/${id}`)}` : null;
  return <DeviceDetail item={item} canAct={canAct} signInHref={signInHref} />;
}
