export type NoticeValue = { kind: "ok" | "error"; text: string } | null;

export function Notice({ notice }: { notice: NoticeValue }) {
  if (!notice) return null;
  return notice.kind === "error"
    ? <p role="alert" className="alert">{notice.text}</p>
    : <p role="status" className="ok">{notice.text}</p>;
}
