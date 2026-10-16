import type { Metadata } from "next";
import { headers } from "next/headers";

import { CenteredPage } from "@orchester/ui/components/frame";

import { AdminBrand, CardHead, EntryCard } from "@/components/page";
import { t } from "@/lib/i18n";
import { originOf, placeOf } from "@/lib/server/place";

import styles from "./address.module.css";

export const metadata: Metadata = { title: t("address.pageTitle") };

/**
 * A project's own address, `/<slug>` (`j8WujU`, *A project's own address*): *Open this in Orchester*. It names nothing
 * and asks the server nothing, so it says the same for a project that exists and one that does not.
 */
export default async function ProjectAddress({ params }: { params: Promise<{ slug: string }> }) {
  const { slug } = await params;
  const address = `${originOf(placeOf(await headers()))}/${slug}`;
  return (
    <CenteredPage brand={<AdminBrand />} footer={null}>
      <EntryCard>
        <CardHead title={t("address.title")} description={t("address.text", { address })} />
        <a className={styles.link} href="/admin">
          {t("address.admin")}
        </a>
      </EntryCard>
    </CenteredPage>
  );
}
