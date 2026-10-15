import type { Metadata } from "next";

import { Submit } from "@orchester/ui/components/submit";

import { CardHead, EntryCard, EntryShell } from "@/components/page";
import { t } from "@/lib/i18n";
import { loadServer } from "@/lib/server/admin";
import { nothingFlow } from "@/lib/server/visitor";

import { leaveAction } from "./actions";

export const metadata: Metadata = { title: t("nothing.pageTitle") };

/**
 * *Nothing to manage here* (`j8WujU`): the cloud vouched for the person, but they neither administer the server nor
 * own a project on it. The name and email come from the cloud's token, read for this page only.
 */
export default async function NothingToManage() {
  const server = await loadServer();
  const flow = await nothingFlow();
  const name = flow?.name ?? null;
  const email = flow?.email ?? null;
  const text = name !== null && email !== null
    ? t("nothing.text", { name, email, server: server.name })
    : t("nothing.textNoName", { server: server.name });
  return (
    <EntryShell server={server}>
      <EntryCard>
        <CardHead title={t("nothing.title")} description={text} />
        <form action={leaveAction}>
          <Submit kind="secondary" row icon="log-out" label={t("nothing.signOut")} busyLabel={t("nothing.signOut")} />
        </form>
      </EntryCard>
    </EntryShell>
  );
}
