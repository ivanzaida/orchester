import type { Metadata } from "next";

import { ButtonLink } from "@orchester/ui/components/button";

import { FirstVisitCheck } from "@/components/certificate";
import { CardHead, EntryCard, EntryShell, Pair } from "@/components/page";
import { SetupSteps } from "@/components/setup-steps";
import { message, t } from "@/lib/i18n";
import { loadServer } from "@/lib/server/admin";
import { formToken } from "@/lib/server/visitor";

import { SetupCodeForm } from "./form";
import styles from "./setup.module.css";

export const metadata: Metadata = { title: t("setup.pageTitle") };

/** Setup step 1 (`t9q0y`), or *already set up* (`j8WujU`) once the server has an admin. */
export default async function SetupPage({ searchParams }: { searchParams: Promise<{ expired?: string }> }) {
  const server = await loadServer();
  if (!server.needsSetup) {
    return (
      <EntryShell server={server}>
        <EntryCard>
          <CardHead title={t("setup.alreadyTitle", { server: server.name })} description={t("setup.alreadyText")} />
          <ButtonLink kind="primary" href="/admin/signin">
            {t("setup.signIn")}
          </ButtonLink>
        </EntryCard>
      </EntryShell>
    );
  }
  const { expired } = await searchParams;
  return (
    <EntryShell server={server}>
      <div className={styles.column}>
        <SetupSteps at={1} />
        <Pair>
          <EntryCard size="wide">
            <CardHead title={t("setup.title")} description={t("setup.description")} />
            <SetupCodeForm csrf={await formToken()} notice={expired === undefined ? null : message("setup.codeGone")} />
          </EntryCard>
          <FirstVisitCheck server={server} />
        </Pair>
      </div>
    </EntryShell>
  );
}
