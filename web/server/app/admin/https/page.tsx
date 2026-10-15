import type { Metadata } from "next";
import { headers } from "next/headers";

import { CenteredPage } from "@orchester/ui/components/frame";

import { AdminBrand, CardHead, EntryCard, Small } from "@/components/page";
import { t } from "@/lib/i18n";
import { originOf, placeOf } from "@/lib/server/place";

export const metadata: Metadata = { title: t("https.pageTitle") };

/**
 * *This page needs HTTPS* (`j8WujU`): `proxy.ts` shows it for every admin path reached over plain HTTP anywhere but
 * the server itself, since the admin's cookies are `Secure`.
 */
export default async function HttpsRequired() {
  const place = placeOf(await headers());
  const secure = `${originOf({ ...place, proto: "https" })}/admin`;
  return (
    <CenteredPage brand={<AdminBrand />} footer={null}>
      <EntryCard>
        <CardHead title={t("https.title")} description={t("https.text", { origin: originOf(place) })} />
        <Small>{t("https.open", { secure })}</Small>
      </EntryCard>
    </CenteredPage>
  );
}
