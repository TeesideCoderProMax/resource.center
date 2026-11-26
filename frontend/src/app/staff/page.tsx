import { redirect } from "next/navigation";
import { AccessNotice } from "@/components/AccessNotice";
import { access } from "@/lib/roles";
import { getViewer } from "@/lib/viewer";

export const dynamic = "force-dynamic";

export default async function StaffPage() {
  const a = access(await getViewer(), "staff");
  if (a !== "ok") return <AccessNotice access={a} returnTo="/staff/dashboard" />;
  redirect("/staff/dashboard");
}
