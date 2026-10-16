import { EmptyState, Panel, Section } from "@orchester/ui/components/list";

import { t } from "@/lib/i18n";
import type { Invite } from "@/lib/model";

import { InviteRow } from "./invite-row";

/** Open invites first, newest first; then the ones that ended in the last 30 days, newest first. */
export function inviteOrder(invites: readonly Invite[]): Invite[] {
  return [...invites].sort((a, b) => Number(b.state === "open") - Number(a.state === "open") || b.createdAt - a.createdAt);
}

/**
 * *Open and recent invites* (`LZdYm`): open ones with *Revoke*, then those that ended in the last 30 days. Without a
 * `title` it is the list alone, under the form that makes them (`Gs9Vu`, invites to the server).
 */
export function InviteList(props: {
  id: string;
  title?: string;
  description?: string;
  invites: readonly Invite[];
  empty: { title: string; text: string } | null;
  csrf: string;
  server: string;
  now: number;
}) {
  const open = props.invites.filter((invite) => invite.state === "open").length;
  if (props.invites.length === 0 && props.empty === null) {
    return null;
  }
  const panel = (
    <Panel>
      {props.invites.length === 0 && props.empty !== null ? (
        <EmptyState icon="mail" title={props.empty.title} text={props.empty.text} />
      ) : (
        inviteOrder(props.invites).map((invite) => (
          <InviteRow key={invite.id} invite={invite} csrf={props.csrf} server={props.server} now={props.now} />
        ))
      )}
    </Panel>
  );
  if (props.title === undefined) {
    return <div id={props.id}>{panel}</div>;
  }
  return (
    <Section id={props.id} title={props.title} meta={open > 0 ? t("invites.open", { count: open }) : undefined} description={props.description}>
      {panel}
    </Section>
  );
}
