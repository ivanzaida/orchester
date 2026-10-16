import type { Metadata } from "next";

import { ButtonLink } from "@orchester/ui/components/button";
import { ErrorLine } from "@orchester/ui/components/status";

import { LinkForm } from "@/components/link-form";
import { LinkOutcome } from "@/components/link-outcome";
import { CardHead, EntryCard, EntryShell, Small } from "@/components/page";
import { SetupSteps } from "@/components/setup-steps";
import { t } from "@/lib/i18n";
import { loadAdmin } from "@/lib/server/admin";
import { config } from "@/lib/server/config";
import { formToken } from "@/lib/server/visitor";

import styles from "../setup.module.css";
import cloudStyles from "./cloud.module.css";

export const metadata: Metadata = { title: t("setup.pageTitle") };

/** Setup step 3 (`CeBNR`): link the server to Orchester Cloud through the browser, or not now. */
export default async function SetupCloudPage({ searchParams }: { searchParams: Promise<{ link?: string; name?: string }> }) {
  const context = await loadAdmin();
  const { server } = context;
  const { link, name } = await searchParams;
  const cloud = config().cloudOrigin;
  return (
    <EntryShell server={server}>
      <div className={styles.column}>
        <SetupSteps at={3} />
        <EntryCard size="wide">
          <CardHead title={t("setup.cloudTitle", { server: server.name })} description={t("setup.cloudText", { server: server.name })} />
          {name === "unsaved" && <ErrorLine>{t("setup.nameNotSaved")}</ErrorLine>}
          <LinkOutcome outcome={link} server={server.name} />
          {cloud === null ? (
            <Small>{t("setup.noCloudText", { server: server.name })}</Small>
          ) : (
            <>
              <LinkForm csrf={await formToken()} back="setup" kind="primary" />
              <Small>{t("setup.cloudHint", { host: new URL(cloud).host, server: server.name })}</Small>
            </>
          )}
          <div className={cloudStyles.notNow}>
            <span className={cloudStyles.question}>{t("setup.noInternet")}</span>
            <ButtonLink kind="ghost" row href="/admin/setup/done">
              {t("setup.notNow")}
            </ButtonLink>
          </div>
          <Small>{t("setup.notNowHint", { server: server.name })}</Small>
        </EntryCard>
      </div>
    </EntryShell>
  );
}
