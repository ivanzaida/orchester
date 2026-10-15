"use client";

import { ActionButton } from "@orchester/ui/components/button";
import { CodeShownOnce } from "@orchester/ui/components/code-once";
import { useTimeZone } from "@orchester/ui/hooks/browser";

import { clockTime, shortDate } from "@/lib/dates";
import { t } from "@/lib/i18n";

import styles from "./invites.module.css";

/**
 * A code shown once (`bdPlP`), with the address and check code the person needs, and *Done, I've given it*, which
 * drops it from the page for good. It came in the answer to the action that made it, and lives only here.
 */
export function CodeCard(props: {
  title: string;
  code: string;
  address: string;
  checkCode: string | null;
  expiresAt: number;
  hours: number;
  warning: string;
  done: () => void;
}) {
  const zone = useTimeZone();
  const when = t("common.dateTime", { date: shortDate(props.expiresAt, zone), time: clockTime(props.expiresAt, zone) });
  const facts = [
    { label: t("invites.address"), value: props.address, mono: true },
    { label: t("invites.checkCode"), value: props.checkCode ?? t("invites.noCheckCode"), mono: props.checkCode !== null },
    { label: t("invites.expires"), value: t("invites.expiresValue", { when, hours: props.hours }) },
  ];
  return (
    <div className={styles.codeArea}>
      <CodeShownOnce title={props.title} badge={t("common.shownOnce")} code={props.code} copyLabel={t("common.copyCode")}
        copiedLabel={t("common.copied")} facts={facts} warning={props.warning} />
      <ActionButton kind="secondary" row onClick={props.done}>
        {t("common.done")}
      </ActionButton>
    </div>
  );
}
