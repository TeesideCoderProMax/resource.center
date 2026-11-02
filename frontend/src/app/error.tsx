"use client";

export default function Error({ error, reset }: { error: Error & { digest?: string }; reset: () => void }) {
  return (
    <section className="panel">
      <h1>Something went wrong</h1>
      <p role="alert" className="alert">The page could not be shown.{error.digest ? ` (ref ${error.digest})` : ""}</p>
      <button className="btn" onClick={reset}>Try again</button>
    </section>
  );
}
