import type { Metadata } from "next";
import "./globals.css";

export const metadata: Metadata = {
  title: "Campus Device Loan — Resource Center",
  description:
    "Distributed 2-day campus device loan catalogue and reservation portal",
};

export default function RootLayout({
  children,
}: Readonly<{
  children: React.ReactNode;
}>) {
  return (
    <html lang="en">
      <body>
        {children}
      </body>
    </html>
  );
}
