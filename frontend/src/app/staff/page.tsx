import { AccessNotice } from "@/components/AccessNotice";
import { StaffDesk } from "@/components/StaffDesk";
import { access } from "@/lib/roles";
import { fetchCatalogue, getViewer } from "@/lib/viewer";

export const dynamic = "force-dynamic";

export default async function StaffPage() {
  const a = access(await getViewer(), "staff");
  if (a !== "ok") return <AccessNotice access={a} returnTo="/staff" />;
  const { items } = await fetchCatalogue();
  return <StaffDesk catalogue={items} />;
}
