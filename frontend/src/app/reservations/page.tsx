import { AccessNotice } from "@/components/AccessNotice";
import { MyReservations } from "@/components/ReservationTable";
import { access, isStaff } from "@/lib/roles";
import { getViewer } from "@/lib/viewer";

export const dynamic = "force-dynamic";

export default async function ReservationsPage() {
  const viewer = await getViewer();
  const a = access(viewer, "member");
  if (a !== "ok") return <AccessNotice access={a} returnTo="/reservations" />;
  return isStaff(viewer)
    ? <MyReservations title="All reservations" showStudent />
    : <MyReservations title="My reservations" />;
}
