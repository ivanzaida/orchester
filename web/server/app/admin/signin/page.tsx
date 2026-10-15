import type { Metadata } from "next";
import { redirect } from "next/navigation";

import { ButtonLink } from "@orchester/ui/components/button";
import { Callout } from "@orchester/ui/components/callout";
import { ErrorLine } from "@orchester/ui/components/status";
import { Submit } from "@orchester/ui/components/submit";

import { FirstVisitCheck } from "@/components/certificate";
import { CsrfField, Hidden } from "@/components/hidden";
import { CardHead, EntryCard, EntryShell, Pair, Small } from "@/components/page";
import { t, type MessageKey } from "@/lib/i18n";
import type { ServerInfo } from "@/lib/model";
import { loadServer } from "@/lib/server/admin";
import { config } from "@/lib/server/config";
import { formToken, session } from "@/lib/server/visitor";

import styles from "./signin.module.css";

export const metadata: Metadata = { title: t("signin.pageTitle") };

/** What the trip back from Orchester Cloud ended with, as the sign-in page says it. Only these fixed words. */
const FAILURES: Readonly<Record<string, MessageKey>> = {
  not_invited: "signin.notInvited",
  invalid_identity: "signin.invalidIdentity",
  key_set_stale: "signin.keySetStale",
  cloud_off: "signin.cloudOff",
  rate_limited: "signin.tooMany",
  trip: "signin.tripLost",
  expired: "error.formExpired",
  unavailable: "signin.failed",
};

/** *Sign in with Orchester Cloud*, or why it is not offered now (`j8WujU`). */
function CloudSignIn({ server, csrf }: { server: ServerInfo; csrf: string }) {
  const cloud = config().cloudOrigin;
  if (cloud === null) {
    return <Callout tone="neutral" icon="cloud-off" title={t("signin.noCloudTitle")} body={t("signin.noCloudText")} />;
  }
  if (server.cloud === null) {
    return <Callout tone="warning" icon="cloud-off" title={t("signin.unlinkedTitle", { server: server.name })} body={t("signin.unlinkedText")} />;
  }
  if (!server.cloud.accepting) {
    return <Callout tone="warning" icon="cloud-off" title={t("signin.cloudOffTitle")} body={t("signin.cloudOffText")} />;
  }
  return (
    <>
      <form action="/admin/trip" method="post">
        <CsrfField token={csrf} />
        <Hidden name="purpose" value="admin" />
        <Submit kind="primary" icon="cloud" label={t("signin.cloud")} busyLabel={t("signin.cloudBusy")} />
      </form>
      <Small>{t("signin.cloudHint", { host: new URL(cloud).host, server: server.name })}</Small>
    </>
  );
}

/** Sign in (`DJUEA`): with Orchester Cloud through the browser, or with a code; the certificate check beside it. */
export default async function SignInPage({ searchParams }: { searchParams: Promise<{ failed?: string }> }) {
  const server = await loadServer();
  if (server.needsSetup) {
    redirect("/admin/setup");
  }
  if ((await session()) !== null) {
    redirect("/admin");
  }
  const { failed } = await searchParams;
  const failure = failed === undefined ? undefined : FAILURES[failed];
  const cloudReady = config().cloudOrigin !== null && server.cloud?.accepting === true;
  return (
    <EntryShell server={server}>
      <Pair>
        <EntryCard>
          <CardHead title={t("signin.title", { server: server.name })} description={t("signin.description")} />
          {failure !== undefined && <ErrorLine>{t(failure, { server: server.name })}</ErrorLine>}
          <CloudSignIn server={server} csrf={await formToken()} />
          {cloudReady && (
            <div className={styles.or}>
              <span className={styles.line} />
              <span>{t("signin.or")}</span>
              <span className={styles.line} />
            </div>
          )}
          <ButtonLink kind={cloudReady ? "secondary" : "primary"} icon="key-round" href="/admin/signin/code">
            {t("signin.code")}
          </ButtonLink>
          {cloudReady && <Small>{t("signin.codeHint")}</Small>}
        </EntryCard>
        <FirstVisitCheck server={server} />
      </Pair>
    </EntryShell>
  );
}
