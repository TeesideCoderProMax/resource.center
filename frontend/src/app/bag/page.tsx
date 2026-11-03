import { AccessNotice } from "@/components/AccessNotice";
import { LoanBag } from "@/components/LoanBag";
import { access, roleLabel } from "@/lib/roles";
import { fetchCatalogue, getViewer } from "@/lib/viewer";

export const dynamic = "force-dynamic";

export default async function BagPage() {
  const [viewer, { items, error }] = await Promise.all([getViewer(), fetchCatalogue()]);
  const a = access(viewer, "member");
  if (a !== "ok") return <AccessNotice access={a} returnTo="/bag" />;
  return <LoanBag catalogue={items} catalogueError={error} email={viewer.email} role={roleLabel(viewer)} />;
}
