import type { Metadata } from "next";

import { Icon } from "@orchester/ui/components/icon";
import { EmptyState, Panel, Section } from "@orchester/ui/components/list";

import { Note, PageHead } from "@/components/page-head";
import { AdminShell } from "@/components/shell";
import { t } from "@/lib/i18n";
import type { Approval, Device } from "@/lib/model";
import { loadAdmin, value } from "@/lib/server/admin";
import { approvals } from "@/lib/server/team/auth";
import { formToken } from "@/lib/server/visitor";

import { ApprovalRow, DeviceRow } from "./rows";
import styles from "./devices.module.css";

export const metadata: Metadata = { title: t("devices.pageTitle") };

function matches(query: string, ...texts: string[]): boolean {
  return query === "" || texts.some((text) => text.toLowerCase().includes(query));
}

function computers(count: number): string {
  return count === 1 ? t("devices.computersOne") : t("devices.computers", { count });
}

/** The search (`N8wkG`): a plain `GET` form, so it works without script and the address can be shared. */
function Search({ query }: { query: string }) {
  return (
    <form method="get" role="search">
      <label className={styles.search}>
        <Icon name="search" size="note" />
        <input className={styles.searchInput} type="search" name="q" defaultValue={query} placeholder={t("devices.search")}
          aria-label={t("devices.search")} />
      </label>
    </form>
  );
}

function Waiting({ list, csrf, now }: { list: readonly Approval[]; csrf: string; now: number }) {
  if (list.length === 0) {
    return null;
  }
  return (
    <Section id="waiting" title={t("devices.waitingTitle")} meta={computers(list.length)} description={t("devices.waitingText")}>
      <div className={styles.waiting}>
        {list.map((approval) => <ApprovalRow key={approval.id} approval={approval} csrf={csrf} now={now} />)}
      </div>
    </Section>
  );
}

function Joined(props: { list: readonly Device[]; query: string; me: string; server: string; csrf: string; now: number }) {
  const empty = props.query === ""
    ? { title: t("devices.emptyTitle"), text: t("devices.emptyText", { server: props.server }) }
    : { title: t("devices.noMatchTitle", { query: props.query }), text: t("devices.noMatchText") };
  return (
    <Section id="joined" title={t("devices.joinedTitle")} meta={computers(props.list.length)}>
      <div className={styles.stack}>
        <Panel>
          {props.list.length === 0 ? (
            <EmptyState icon="laptop" title={empty.title} text={empty.text} />
          ) : (
            props.list.map((device) => (
              <DeviceRow key={device.id} device={device} me={props.me} server={props.server} csrf={props.csrf} now={props.now} />
            ))
          )}
        </Panel>
        <Note>{t("devices.note")}</Note>
      </div>
    </Section>
  );
}

/** Devices (`YY8pR`; approve, refuse and revoke in `WV4AT`): computers waiting for approval, and every joined one. */
export default async function DevicesPage({ searchParams }: { searchParams: Promise<{ q?: string }> }) {
  const context = await loadAdmin();
  const { server, devices } = context;
  if (devices === null) {
    return (
      <AdminShell context={context} area="devices">
        <Panel>
          <EmptyState icon="lock" title={t("devices.adminsOnlyTitle")} text={t("devices.adminsOnlyText", { server: server.name })} />
        </Panel>
      </AdminShell>
    );
  }
  const query = ((await searchParams).q ?? "").trim().toLowerCase();
  const waiting = value(await approvals(context.options)).filter((a) => matches(query, a.device, a.user.displayName));
  const joined = devices.filter((d) => matches(query, d.device, d.user.displayName, d.user.username));
  const csrf = await formToken();
  const now = Date.now();
  return (
    <AdminShell context={context} area="devices">
      <PageHead title={t("devices.title")} description={t("devices.description", { server: server.name })} actions={<Search query={query} />} />
      <Waiting list={waiting} csrf={csrf} now={now} />
      <Joined list={joined} query={query} me={context.me.id} server={server.name} csrf={csrf} now={now} />
    </AdminShell>
  );
}
