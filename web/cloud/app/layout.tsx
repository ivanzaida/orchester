import type { Metadata, Viewport } from "next";
import { connection } from "next/server";
import type { ReactNode } from "react";

import { DEFAULT_LOCALE, t } from "@/lib/i18n";

import "@orchester/ui/theme/index.css";

export const metadata: Metadata = {
  title: { template: t("brand.pageTitle"), default: t("brand.home") },
  referrer: "strict-origin",
  robots: { index: false, follow: false },
};

export const viewport: Viewport = {
  width: "device-width",
  initialScale: 1,
  colorScheme: "dark light",
};

export default async function RootLayout({ children }: { children: ReactNode }) {
  // Every page is rendered per request: its scripts carry the request's CSP nonce, and its forms the visitor's token.
  await connection();
  return (
    <html lang={DEFAULT_LOCALE}>
      <body>{children}</body>
    </html>
  );
}
