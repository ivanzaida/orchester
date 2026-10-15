import type { Metadata } from "next";
import { headers } from "next/headers";

import { CenteredPage } from "@orchester/ui/components/frame";

import { AdminBrand, CardHead, EntryCard, Small } from "@/components/page";
import { t } from "@/lib/i18n";
import { originOf, placeOf } from "@/lib/server/place";

export const metadata: Metadata = { title: t("https.pageTitle") };

/** The origin `proxy.ts` puts in the internal address of the rewrite; anything else falls back to the request's. */
const ORIGIN = /^http:\/\/[A-Za-z0-9.-]+(?::\d{1,5})?$|^http:\/\/\[[0-9A-Fa-f:.]+\](?::\d{1,5})?$/;

/**
 * *This page needs HTTPS* (`j8WujU`): `proxy.ts` shows it for every admin path reached over plain HTTP anywhere but
 * the server itself, since the admin's cookies are `Secure`.
 */
export default async function HttpsRequired({ searchParams }: { searchParams: Promise<{ origin?: string }> }) {
  const sent = (await searchParams).origin;
  const origin = sent !== undefined && ORIGIN.test(sent) ? sent : originOf(placeOf(await headers()));
  const secure = `${origin.replace(/^http:/, "https:")}/admin`;
  return (
    <CenteredPage brand={<AdminBrand />} footer={null}>
      <EntryCard>
        <CardHead title={t("https.title")} description={t("https.text", { origin })} />
        <Small>{t("https.open", { secure })}</Small>
      </EntryCard>
    </CenteredPage>
  );
}
