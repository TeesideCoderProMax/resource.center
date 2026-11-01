import type { Metadata } from "next";

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
      <body
        style={{
          margin: 0,
          fontFamily:
            "system-ui, -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif",
          backgroundColor: "#0f172a",
          color: "#f8fafc",
          lineHeight: 1.5,
        }}
      >
        {children}
      </body>
    </html>
  );
}
