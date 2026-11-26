import { AccessNotice } from "@/components/AccessNotice";
import { StaffInventory } from "@/components/StaffInventory";
import { access } from "@/lib/roles";
import { getViewer } from "@/lib/viewer";

export const dynamic = "force-dynamic";

export default async function StaffInventoryPage() {
  const a = access(await getViewer(), "staff");
  if (a !== "ok") return <AccessNotice access={a} returnTo="/staff/inventory" />;
  return <StaffInventory />;
}
