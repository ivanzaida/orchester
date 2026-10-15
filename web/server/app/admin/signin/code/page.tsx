import type { Metadata } from "next";
import { redirect } from "next/navigation";

import { SwitchLine } from "@orchester/ui/components/frame";

import { CardHead, EntryCard, EntryShell } from "@/components/page";
import { t } from "@/lib/i18n";
import { loadServer } from "@/lib/server/admin";
import { config } from "@/lib/server/config";
import { formToken, session } from "@/lib/server/visitor";

import { CodeForm } from "./form";

export const metadata: Metadata = { title: t("code.pageTitle") };

/** Sign in with a code (`ulIoG`): a pairing code from Orchester, or an account code printed on the server. */
export default async function CodeSignInPage() {
  const server = await loadServer();
  if (server.needsSetup) {
    redirect("/admin/setup");
  }
  if ((await session()) !== null) {
    redirect("/admin");
  }
  const cloudOffered = config().cloudOrigin !== null && server.cloud?.accepting === true;
  return (
    <EntryShell server={server}>
      <EntryCard>
        <CardHead title={t("code.title")} description={t("code.description", { server: server.name })} />
        <CodeForm csrf={await formToken()} />
      </EntryCard>
      {cloudOffered && <SwitchLine question={t("code.question")} link={t("code.link")} href="/admin/signin" />}
    </EntryShell>
  );
}
