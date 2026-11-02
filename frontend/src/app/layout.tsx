import type { Metadata } from "next";
import { AppHeader } from "@/components/AppHeader";
import { getViewer } from "@/lib/viewer";
import "./globals.css";

export const metadata: Metadata = {
  title: "Campus Device Loan — Resource Center",
  description:
    "Distributed 2-day campus device loan catalogue and reservation portal",
};

export const dynamic = "force-dynamic";

export default async function RootLayout({
  children,
}: Readonly<{
  children: React.ReactNode;
}>) {
  const viewer = await getViewer();
  const envName = process.env.APP_ENV_NAME || "local";
  return (
    <html lang="en">
      <body>
        <a href="#main" className="skip">Skip to content</a>
        <div className="wrap">
          <AppHeader viewer={viewer} />
          <main id="main">{children}</main>
          <footer className="foot muted small">Resource Center · environment: {envName}</footer>
        </div>
      </body>
    </html>
  );
}
