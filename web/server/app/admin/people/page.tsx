import type { Metadata } from "next";

import { EmptyState, Panel, Section } from "@orchester/ui/components/list";

import { InviteList } from "@/components/invites/invite-list";
import { Note, PageHead } from "@/components/page-head";
import { AdminShell } from "@/components/shell";
import { t } from "@/lib/i18n";
import { loadAdmin, value } from "@/lib/server/admin";
import { originOf } from "@/lib/server/place";
import { emailReach } from "@/lib/server/reach";
import { invites } from "@/lib/server/team/invites";
import { formToken } from "@/lib/server/visitor";

import { PeopleTable } from "./people";
import styles from "./people.module.css";
import { ServerInvite } from "./server-invite";

export const metadata: Metadata = { title: t("people.pageTitle") };

/** People (`waaNU`; its states in `Gs9Vu`): accounts on the server, and invites to the server without a project. */
export default async function PeoplePage() {
  const context = await loadAdmin();
  const { server, me, people } = context;
  if (people === null) {
    return (
      <AdminShell context={context} area="people">
        <Panel>
          <EmptyState icon="lock" title={t("people.adminsOnlyTitle")} text={t("people.adminsOnlyText", { server: server.name })} />
        </Panel>
      </AdminShell>
    );
  }
  const serverInvites = value(await invites(null, context.options)).filter((invite) => invite.project === null);
  const csrf = await formToken();
  const own = originOf(context.place);
  const count = people.length === 1 ? t("people.countOne") : t("people.count", { count: people.length });
  return (
    <AdminShell context={context} area="people">
      <PageHead title={t("people.title")} description={t("people.description", { server: server.name })} />
      <ServerInvite csrf={csrf} server={server.name} reach={emailReach(server)} address={own}
        checkCode={server.certificate?.checkCode ?? null} />
      <InviteList id="server-invites" invites={serverInvites} empty={null} csrf={csrf} server={server.name} now={Date.now()} />
      <Section id="accounts" title={t("people.accounts")} meta={count}>
        <div className={styles.stack}>
          <PeopleTable people={people} me={me.id} server={server.name} address={own} checkCode={server.certificate?.checkCode ?? null}
            csrf={csrf} />
          <Note>{t("people.note")}</Note>
        </div>
      </Section>
    </AdminShell>
  );
}
