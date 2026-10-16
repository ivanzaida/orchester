"use client";

import { useActionState } from "react";

import { AddressLine, Chip, EmptyState, Panel, Row, RowLine, Section, TopLine } from "@orchester/ui/components/list";
import { Note } from "@orchester/ui/components/card";
import { Submit } from "@orchester/ui/components/submit";

import { ErrorLine } from "@/components/fields";
import { CsrfField, Hidden } from "@/components/hidden";
import { InviteDates, Since } from "@/components/moment";
import type { Invite, Listing, Server } from "@/lib/account";
import { t } from "@/lib/i18n";

import { declineAction, type FormState } from "./actions";
import styles from "./account.module.css";

/**
 * *Invites* and *Your servers* (`usaZQ`, `YNIjc`): the cloud's invites in flight and its server registry. Each row has
 * *Decline*; *Accept in Orchester* waits for the `orchester://` scheme the desktop does not register yet.
 */
function Decline({ csrf, id }: { csrf: string; id: string }) {
  const [state, action] = useActionState<FormState, FormData>(declineAction, {});
  return (
    <form action={action}>
      <CsrfField token={csrf} />
      <Hidden name="invite" value={id} />
      <Submit kind="ghost" row label={t("account.invites.decline")} busyLabel={t("account.invites.declining")} />
      {state.error && <ErrorLine message={state.error} />}
    </form>
  );
}

function InviteRow({ invite, csrf, now }: { invite: Invite; csrf: string; now: number }) {
  const chip =
    invite.role === null
      ? t("account.invites.kindServer")
      : t("account.invites.kindProject", { role: t(`account.invites.roles.${invite.role}`) });
  const from =
    invite.project === null
      ? t("account.invites.fromServer", { inviter: invite.inviterName })
      : t("account.invites.fromProject", { inviter: invite.inviterName, server: invite.serverName });
  return (
    <Row glyph={invite.project === null ? "server" : "folder-git-2"} align="start" fill side={<Decline csrf={csrf} id={invite.id} />}>
      <TopLine name={invite.project?.name ?? invite.serverName} weight="semibold">
        <Chip>{chip}</Chip>
      </TopLine>
      <RowLine tone="secondary" wide>
        {from}
      </RowLine>
      <AddressLine>{invite.project?.address ?? invite.serverAddress}</AddressLine>
      <RowLine>
        <InviteDates expiresAt={invite.expiresAt} now={now} />
      </RowLine>
    </Row>
  );
}

export function Invites(props: { listing: Listing<Invite>; email: string; csrf: string; now: number }) {
  const { listing } = props;
  const count = listing.kind === "list" && listing.items.length > 0 ? listing.items.length : undefined;
  return (
    <Section
      id="invites"
      title={t("account.invites.title")}
      meta={count === undefined ? undefined : t("account.invites.waiting", { count })}
      description={t("account.invites.description")}
    >
      <Panel>
        {listing.kind === "list" && listing.items.length > 0 ? (
          listing.items.map((invite) => <InviteRow key={invite.id} invite={invite} csrf={props.csrf} now={props.now} />)
        ) : (
          <EmptyState icon="mail" {...emptyWords("invites", listing.kind, props.email)} />
        )}
      </Panel>
    </Section>
  );
}

function emptyWords(area: "invites" | "servers", kind: Listing<unknown>["kind"], email: string) {
  if (kind === "failed") {
    return { title: t(`account.${area}.failedTitle`), text: t(`account.${area}.failedText`) };
  }
  return { title: t(`account.${area}.emptyTitle`), text: t(`account.${area}.emptyText`, { email }) };
}

function ServerRow({ server }: { server: Server }) {
  const side = (
    <span className={styles.serverSide}>
      <RowLine>
        <Since at={server.since} owner={server.owner} />
      </RowLine>
      <RowLine tone="secondary">{t("account.servers.projects")}</RowLine>
    </span>
  );
  return (
    <Row glyph="server" align="center" side={side}>
      <TopLine name={server.name} weight="semibold">
        {server.owner && <Chip>{t("account.servers.owned")}</Chip>}
      </TopLine>
      <AddressLine>{server.address}</AddressLine>
    </Row>
  );
}

export function Servers({ listing, email }: { listing: Listing<Server>; email: string }) {
  const count = listing.kind === "list" ? listing.items.length : 0;
  const meta = count === 1 ? t("account.servers.countOne") : t("account.servers.count", { count });
  return (
    <Section id="servers" title={t("account.servers.title")} meta={count > 0 ? meta : undefined} description={t("account.servers.description")}>
      <Panel>
        {listing.kind === "list" && count > 0 ? (
          listing.items.map((server) => <ServerRow key={server.id} server={server} />)
        ) : (
          <EmptyState icon="server" {...emptyWords("servers", listing.kind, email)} />
        )}
      </Panel>
      {count > 0 && <Note icon="info">{t("account.servers.note")}</Note>}
    </Section>
  );
}
