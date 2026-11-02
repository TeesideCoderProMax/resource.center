"use client";

import { useRouter } from "next/navigation";
import { DeviceCard } from "@/components/DeviceCard";
import type { DeviceModelSummary } from "@/lib/loan";

export function DeviceDetail({ item, canAct }: { item: DeviceModelSummary; canAct: boolean }) {
  const router = useRouter();
  return <DeviceCard item={item} canAct={canAct} detail onChanged={() => router.refresh()} />;
}
