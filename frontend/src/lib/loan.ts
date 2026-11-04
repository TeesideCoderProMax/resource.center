/** Loan API types (docs/backend/contracts.md) and shared helpers. */
export interface DeviceModelSummary {
  id: string;
  name: string;
  category: string;
  description: string;
  /** Maximum period; the borrower chooses the actual reservation length. */
  loanDurationDays: number;
  imageUrl?: string | null;
  imageAlt?: string | null;
  specifications?: { label: string; value: string }[];
  totalUnits: number;
  availableUnits: number;
}

export interface ReservationRow {
  reservationId: string;
  modelId: string;
  unitId: string;
  studentSub: string;
  studentEmail: string;
  status: "RESERVED" | "COLLECTED" | "RETURNED";
  reservedAt: string;
  dueAt: string;
  collectedAt: string | null;
  returnedAt: string | null;
}

export interface ApiError {
  error: string;
  message: string;
  correlationId?: string;
}

export function loanApiBaseUrl(): string {
  return (process.env.LOAN_API_BASE_URL ?? process.env.NEXT_PUBLIC_LOAN_API_URL ?? "").replace(/\/$/, "");
}

/**
 * Reads the role claim from an access token for display only. The Loan API verifies the
 * signature and enforces roles; the UI never trusts this value for authorization.
 */
export function displayRoles(accessToken: string | undefined): string[] {
  if (!accessToken) return [];
  const claim = process.env.AUTH0_ROLES_CLAIM || "https://resource-center.campus/roles";
  try {
    const payload = JSON.parse(Buffer.from(accessToken.split(".")[1], "base64url").toString("utf8"));
    const roles = payload[claim];
    return Array.isArray(roles) ? roles.filter((r): r is string => typeof r === "string").map((r) => r.toLowerCase()) : [];
  } catch {
    return [];
  }
}
