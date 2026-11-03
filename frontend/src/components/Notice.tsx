import { describe, errorHint, type ErrorHint } from "@/lib/api-client";

export type NoticeValue = { kind: "ok" | "error"; text: string; hint?: ErrorHint; returnTo?: string; link?: { href: string; label: string } } | null;

/** Builds an error notice from a caught request error (message, correlation ref and UI hint). */
export function errorNotice(e: unknown, returnTo?: string): NoticeValue {
  return { kind: "error", text: describe(e), hint: errorHint(e), returnTo };
}

export function Notice({ notice }: { notice: NoticeValue }) {
  if (!notice) return null;
  if (notice.kind === "ok") {
    return <p role="status" className="ok">{notice.text}{notice.link && <> <a href={notice.link.href}>{notice.link.label}</a></>}</p>;
  }
  return (
    <div role="alert" className={notice.hint === "degraded" ? "alert degraded" : "alert"}>
      {notice.hint === "forbidden" && <strong>403 Forbidden. </strong>}
      {notice.hint === "degraded" && <strong>Service degraded. </strong>}
      {notice.text}
      {notice.hint === "sign-in" && (
        <> <a href={`/auth/login?returnTo=${encodeURIComponent(notice.returnTo ?? "/")}`}>Sign in again</a></>
      )}
    </div>
  );
}
