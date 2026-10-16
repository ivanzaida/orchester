import type { Metadata } from "next";
import type { ReactNode } from "react";

import { ButtonLink } from "@orchester/ui/components/button";
import { Callout } from "@orchester/ui/components/callout";
import { Facts } from "@orchester/ui/components/facts";
import { Icon } from "@orchester/ui/components/icon";

import { EntryCard, EntryShell } from "@/components/page";
import { SetupSteps } from "@/components/setup-steps";
import { t } from "@/lib/i18n";
import { loadAdmin } from "@/lib/server/admin";
import { originOf } from "@/lib/server/place";
import { cloudLink } from "@/lib/server/team/settings";

import styles from "../setup.module.css";
import doneStyles from "./done.module.css";

export const metadata: Metadata = { title: t("setup.pageTitle") };

function Next({ number, title, text, children }: { number: number; title: string; text: string; children?: ReactNode }) {
  return (
    <li className={doneStyles.step}>
      <span className={doneStyles.number} aria-hidden="true">
        {number}
      </span>
      <div className={doneStyles.stepText}>
        <span className={doneStyles.stepTitle}>{title}</span>
        <span className={doneStyles.stepSummary}>{text}</span>
        {children}
      </div>
    </li>
  );
}

/** The setup is done (`w7qUM`; not linked `j8WujU`): the facts to give the team, then what to do next. */
export default async function SetupDonePage() {
  const context = await loadAdmin();
  const { server, me } = context;
  const link = server.cloud === null ? null : await cloudLink(context.options);
  const linkedBy = link?.ok === true ? link.value.cloud.linkedBy : null;
  const cloudValue = linkedBy === null ? t("setup.linkedNoBy") : t("setup.linkedBy", { name: linkedBy.displayName });
  const facts = [
    { label: t("setup.address"), value: originOf(context.place), mono: true },
    { label: t("setup.checkCode"), value: server.certificate?.checkCode ?? t("setup.noCheckCode"), mono: server.certificate !== null },
    ...(server.cloud === null ? [] : [{ label: t("setup.cloud"), value: cloudValue }]),
    { label: t("setup.you"), value: t("setup.youValue", { username: me.username }) },
  ];
  return (
    <EntryShell server={server}>
      <div className={styles.column}>
        <SetupSteps at={server.cloud === null ? 3 : 4} />
        <EntryCard size="done">
          <h1 className={doneStyles.title}>
            <span className={doneStyles.check}>
              <Icon name="circle-check" size="tile" />
            </span>
            {t("setup.doneTitle", { server: server.name })}
          </h1>
          <Facts items={facts} labels="wide" boxed />
          {server.cloud === null && (
            <Callout tone="neutral" icon="cloud-off" title={t("setup.notLinkedTitle")} body={t("setup.notLinkedText")} />
          )}
          <ol className={doneStyles.steps}>
            <Next number={1} title={t("setup.next1Title")} text={t("setup.next1Text", { host: context.place.host })}>
              <ButtonLink kind="primary" row icon="plus" href="/admin/projects/new">
                {t("projects.new")}
              </ButtonLink>
            </Next>
            <Next number={2} title={t("setup.next2Title")} text={t("setup.next2Text", { server: server.name })} />
            <Next number={3} title={t("setup.next3Title")} text={t("setup.next3Text")} />
          </ol>
        </EntryCard>
      </div>
    </EntryShell>
  );
}
