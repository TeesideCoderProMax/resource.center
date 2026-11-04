import type { Metadata } from "next";
import { AppHeader } from "@/components/AppHeader";
import { BagProvider } from "@/lib/bag";
import { getViewer } from "@/lib/viewer";
import "./globals.css";

export const metadata: Metadata = {
  title: "Campus Device Hub — Resource Center",
  description:
    "Campus device loan catalogue, reservations and staff desk",
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
        <BagProvider>
          <div className="wrap">
            <AppHeader viewer={viewer} />
            <main id="main">{children}</main>
          </div>
          <footer className="foot">
            <div className="foot-inner">
              <span className="foot-brand">Campus Device Hub</span>
              <a className="small" href="/image-credits">Image credits</a>
              <span className="muted small">Resource Center device loans · environment: {envName}</span>
            </div>
          </footer>
        </BagProvider>
      </body>
    </html>
  );
}
