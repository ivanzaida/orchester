import type { Metadata } from "next";
import { headers } from "next/headers";

import { CenteredPage } from "@orchester/ui/components/frame";

import { AdminBrand, CardHead, EntryCard, Small } from "@/components/page";
import { t } from "@/lib/i18n";
import { config } from "@/lib/server/config";
import { HTTPS_ORIGIN_HEADER, originOf, placeOf } from "@/lib/server/place";
import { deriveKey, verify } from "@/lib/server/seal";

export const metadata: Metadata = { title: t("https.pageTitle") };

/**
 * *This page needs HTTPS* (`j8WujU`): `proxy.ts` shows it for every admin path reached over plain HTTP anywhere but
 * the server itself, since the admin's cookies are `Secure`. It names the address `proxy.ts` signed, or this
 * request's own: never one from its address or an unsigned header (security review 1, minor 2).
 */
export default async function HttpsRequired() {
  const list = await headers();
  const sent = list.get(HTTPS_ORIGIN_HEADER);
  const named = sent === null ? null : verify(deriveKey(config().secret, "place"), sent);
  const origin = named !== null && URL.canParse(named) ? new URL(named).origin : originOf(placeOf(list));
  const secure = `${origin.replace(/^http:/, "https:")}/admin`;
  // The server itself, on the port this browser used (`j8WujU`: http://127.0.0.1:8740/admin).
  const port = new URL(origin).port;
  const local = `http://127.0.0.1${port === "" ? "" : `:${port}`}/admin`;
  return (
    <CenteredPage brand={<AdminBrand />} footer={null}>
      <EntryCard>
        <CardHead title={t("https.title")} description={t("https.text", { origin })} />
        <Small>{t("https.open", { secure, local })}</Small>
      </EntryCard>
    </CenteredPage>
  );
}
