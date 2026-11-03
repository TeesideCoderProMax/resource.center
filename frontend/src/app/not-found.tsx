import Link from "next/link";

export default function NotFound() {
  return (
    <section className="panel">
      <h1>Not found</h1>
      <p className="muted">That page or device does not exist.</p>
      <Link className="btn" href="/">Back to catalogue</Link>
    </section>
  );
}
