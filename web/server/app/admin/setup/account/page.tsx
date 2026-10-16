import type { Metadata } from "next";
import { redirect } from "next/navigation";

import { CardHead, EntryCard, EntryShell } from "@/components/page";
import { SetupSteps } from "@/components/setup-steps";
import { t } from "@/lib/i18n";
import { loadServer } from "@/lib/server/admin";
import { formToken, setupCode } from "@/lib/server/visitor";

import { AccountForm } from "./form";
import styles from "../setup.module.css";

export const metadata: Metadata = { title: t("setup.pageTitle") };

/** Setup step 2 (`y9I12Q`), reached only with a checked setup code in its sealed cookie. */
export default async function SetupAccountPage() {
  const server = await loadServer();
  if (!server.needsSetup) {
    redirect("/admin/setup");
  }
  if ((await setupCode()) === null) {
    redirect("/admin/setup?expired=1");
  }
  return (
    <EntryShell server={server}>
      <div className={styles.column}>
        <SetupSteps at={2} />
        <EntryCard size="wide">
          <CardHead title={t("setup.accountTitle")} description={t("setup.accountText")} />
          <AccountForm csrf={await formToken()} serverName={server.name} />
        </EntryCard>
      </div>
    </EntryShell>
  );
}
