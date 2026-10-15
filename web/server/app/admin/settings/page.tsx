import type { Metadata } from "next";

import { Callout } from "@orchester/ui/components/callout";
import { Badge, Section } from "@orchester/ui/components/list";

import { LinkForm } from "@/components/link-form";
import { LinkOutcome } from "@/components/link-outcome";
import { PageHead } from "@/components/page-head";
import { Hint, SettingRow, SettingsPanel, Value } from "@/components/settings-panel";
import { AdminShell } from "@/components/shell";
import { Phrase } from "@/components/when";
import { t } from "@/lib/i18n";
import type { CloudLink, Settings } from "@/lib/model";
import { loadAdmin, value } from "@/lib/server/admin";
import { config } from "@/lib/server/config";
import { cloudLink, settings } from "@/lib/server/team/settings";
import { formToken } from "@/lib/server/visitor";

import { CloudKeyActions, SwitchForm } from "./forms";
import styles from "./settings.module.css";

export const metadata: Metadata = { title: t("settings.pageTitle") };

function Joining({ current, csrf }: { current: Settings; csrf: string }) {
  return (
    <Section id="joining" title={t("settings.joining")}>
      <SettingsPanel>
        <SettingRow label={t("settings.joinApproval")}>
          <SwitchForm csrf={csrf} op="joinApproval" on={current.joinApproval} label={t("settings.joinApproval")}
            onWord={t("settings.on")} offWord={t("settings.off")} />
          <Hint>{current.joinApproval ? t("settings.joinOnHint") : t("settings.joinOffHint")}</Hint>
          {current.joinApproval && <Hint>{t("settings.joinAlways")}</Hint>}
        </SettingRow>
      </SettingsPanel>
    </Section>
  );
}

function NotLinked({ server, csrf }: { server: string; csrf: string }) {
  return (
    <SettingRow label={t("settings.link")}>
      <Badge tone="neutral">{t("settings.notLinked")}</Badge>
      <Hint>{t("settings.notLinkedText", { server })}</Hint>
      {config().cloudOrigin === null ? <Hint>{t("settings.noCloudText")}</Hint> : <LinkForm csrf={csrf} back="settings" kind="secondary" />}
    </SettingRow>
  );
}

function KeyHint({ link, now }: { link: CloudLink; now: number }) {
  if (link.keySetIssuedAt === null || link.keySetExpiresAt === null) {
    return <Hint>{t("settings.keyHintNoList")}</Hint>;
  }
  const dates = { refreshed: { at: link.keySetIssuedAt, format: "near" as const }, until: { at: link.keySetExpiresAt, format: "date" as const } };
  return (
    <Hint>
      <Phrase k="settings.keyHint" dates={dates} now={now} />
    </Hint>
  );
}

/** *Orchester Cloud's keys here are out of date* (`WV4AT`): the server's key list expired. */
function Stale({ link, server, now }: { link: CloudLink; server: string; now: number }) {
  if (link.keySetIssuedAt === null || link.keySetExpiresAt === null || link.keySetExpiresAt > now) {
    return null;
  }
  const dates = { refreshed: { at: link.keySetIssuedAt, format: "date" as const }, expired: { at: link.keySetExpiresAt, format: "date" as const } };
  const body = (
    <>
      {t("settings.staleText", { server })} <Phrase k="settings.staleLine" dates={dates} />
    </>
  );
  return (
    <span className={styles.callout}>
      <Callout tone="warning" icon="triangle-alert" title={t("settings.staleTitle")} body={body} />
    </span>
  );
}

function LinkLine({ link }: { link: CloudLink }) {
  const cloud = config().cloudOrigin;
  const host = cloud === null ? t("settings.cloud") : new URL(cloud).host;
  const by = link.linkedBy;
  return (
    <span className={styles.linkLine}>
      <Badge tone="success">{t("settings.linked")}</Badge>
      {link.linkedAt !== null && by !== null ? (
        <Phrase k="settings.linkedLine" params={{ host, name: by.displayName }} dates={{ date: { at: link.linkedAt, format: "date" } }} />
      ) : (
        t("settings.linkedLineShort", { host })
      )}
    </span>
  );
}

function Linked({ link, server, csrf }: { link: CloudLink; server: string; csrf: string }) {
  const now = Date.now();
  return (
    <>
      <SettingRow label={t("settings.link")}>
        <LinkLine link={link} />
      </SettingRow>
      <SettingRow label={t("settings.signIns")}>
        <SwitchForm csrf={csrf} op="accepting" on={link.accepting} label={t("settings.signIns")} onWord={t("settings.accepted")}
          offWord={t("settings.off")} />
        {link.accepting ? (
          <Hint>{t("settings.acceptingHint")}</Hint>
        ) : (
          <span className={styles.callout}>
            <Callout tone="warning" icon="cloud-off" title={t("settings.offTitle")} body={t("settings.offText")} />
          </span>
        )}
      </SettingRow>
      <SettingRow label={t("settings.cloudKey")}>
        <Stale link={link} server={server} now={now} />
        <span className={styles.key}>{link.rootFingerprint ?? t("settings.noKey")}</span>
        <KeyHint link={link} now={now} />
        <CloudKeyActions csrf={csrf} server={server} />
      </SettingRow>
    </>
  );
}

function Certificate({ current }: { current: Settings }) {
  const certificate = current.certificate;
  const own = certificate?.mode === "self_signed";
  return (
    <Section id="certificate" title={t("settings.certificate")}>
      <SettingsPanel>
        {current.checkCode !== null && certificate !== null ? (
          <>
            <SettingRow label={t("settings.checkCode")}>
              <span className={styles.checkCode}>{current.checkCode}</span>
              <Hint>{t("settings.checkHint")}</Hint>
            </SettingRow>
            <SettingRow label={t("settings.certRow")}>
              <Value>{own ? t("settings.certSelf") : t("settings.certPem")}</Value>
              <Hint>{own ? t("settings.certHintSelf") : t("settings.certHintPem")}</Hint>
            </SettingRow>
          </>
        ) : (
          <SettingRow label={t("settings.certRow")}>
            <Hint>{t("settings.noCert")}</Hint>
          </SettingRow>
        )}
      </SettingsPanel>
    </Section>
  );
}

/** Settings (`JzPl5`; its states in `WV4AT`): join approval, the link to Orchester Cloud, the certificate. */
export default async function SettingsPage({ searchParams }: { searchParams: Promise<{ link?: string }> }) {
  const context = await loadAdmin();
  const { server } = context;
  if (!context.me.admin) {
    return (
      <AdminShell context={context} area="settings">
        <Callout tone="neutral" icon="lock" title={t("settings.adminsOnly", { server: server.name })} />
      </AdminShell>
    );
  }
  const current = value(await settings(context.options));
  const link = server.cloud === null ? null : value(await cloudLink(context.options)).cloud;
  const csrf = await formToken();
  const { link: outcome } = await searchParams;
  return (
    <AdminShell context={context} area="settings" spaced>
      <PageHead title={t("settings.title")} description={t("settings.description", { server: server.name })} />
      <LinkOutcome outcome={outcome} server={server.name} />
      <Joining current={current} csrf={csrf} />
      <Section id="cloud" title={t("settings.cloud")} description={t("settings.cloudText")}>
        <SettingsPanel>
          {link === null ? <NotLinked server={server.name} csrf={csrf} /> : <Linked link={link} server={server.name} csrf={csrf} />}
        </SettingsPanel>
      </Section>
      <Certificate current={current} />
    </AdminShell>
  );
}
