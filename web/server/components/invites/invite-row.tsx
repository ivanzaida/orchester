"use client";

import { useActionState, useState } from "react";

import { ActionButton } from "@orchester/ui/components/button";
import { InPlaceConfirm } from "@orchester/ui/components/confirm";
import { Badge, Chip, DoneRow, Row, RowLine, TopLine, type Tone } from "@orchester/ui/components/list";
import { Submit } from "@orchester/ui/components/submit";

import { revokeInviteAction, type RevokeState } from "@/app/admin/shared/invite-actions";
import { ErrorLine } from "@/components/fields";
import { CsrfField, Hidden } from "@/components/hidden";
import { Phrase } from "@/components/when";
import { t, type MessageKey } from "@/lib/i18n";
import type { Invite, InviteState } from "@/lib/model";

import styles from "./invites.module.css";

const STATES: Readonly<Record<InviteState, { key: MessageKey; tone: Tone }>> = {
  open: { key: "invites.pending", tone: "warning" },
  expired: { key: "invites.expired", tone: "neutral" },
  revoked: { key: "invites.revoked", tone: "danger" },
  accepted: { key: "invites.accepted", tone: "success" },
};

/** The row's name: the address it went to, or who made the code; an ended email invite's address is forgotten. */
function title(invite: Invite): string {
  if (invite.email !== null) {
    return invite.email;
  }
  return invite.delivery === "code" ? t("invites.codeBy", { name: invite.by.displayName }) : t("invites.endedEmail");
}

function chip(invite: Invite): string {
  if (invite.kind === "account") {
    return t("people.chipAccount");
  }
  if (invite.kind === "server") {
    return invite.admin ? t("people.chipServerAdmin") : t("people.chipServer");
  }
  return invite.role === "owner" ? t("common.owner") : t("common.member");
}

function Sent({ invite, now }: { invite: Invite; now: number }) {
  const at = { at: invite.createdAt, format: "dateTime" as const };
  return invite.delivery === "code" ? (
    <Phrase k="invites.byCode" dates={{ when: at }} now={now} />
  ) : (
    <Phrase k="invites.byEmail" params={{ name: invite.by.displayName }} dates={{ when: at }} now={now} />
  );
}

function firstName(displayName: string): string {
  return displayName.trim().split(/\s+/)[0] ?? displayName;
}

/** When it expires, or how it ended and who ended it (`EmAU7`). */
function When({ invite, now }: { invite: Invite; now: number }) {
  const name = invite.endedBy?.displayName ?? invite.by.displayName;
  const ended = { at: invite.endedAt ?? invite.expiresAt, format: "date" as const };
  switch (invite.state) {
    case "open":
      return <Phrase k="invites.expiresOn" dates={{ when: { at: invite.expiresAt, format: "dateTime" } }} now={now} />;
    case "expired":
      return <Phrase k="invites.expiredOn" dates={{ when: { at: invite.expiresAt, format: "dateTime" } }} now={now} />;
    case "revoked":
      // The first name, as drawn (`iLFy5`: *Revoked by Alice, Oct 11*), so the line keeps to its column.
      return <Phrase k="invites.revokedBy" params={{ name: firstName(name) }} dates={{ date: ended }} now={now} />;
    case "accepted":
      return <Phrase k="invites.acceptedBy" params={{ name }} dates={{ date: ended }} now={now} />;
  }
}

function Revoke({ invite, csrf, action, close }: { invite: Invite; csrf: string; action: (form: FormData) => void; close: () => void }) {
  const code = invite.delivery === "code";
  return (
    <InPlaceConfirm
      title={code ? t("invites.revokeTitleCode", { name: invite.by.displayName }) : t("invites.revokeTitle", { email: title(invite) })}
      text={code ? t("invites.revokeTextCode") : t("invites.revokeText")}
    >
      <ActionButton kind="secondary" row autoFocus onClick={close}>
        {t("invites.keepIt")}
      </ActionButton>
      <form action={action}>
        <CsrfField token={csrf} />
        <Hidden name="invite" value={invite.id} />
        <Submit kind="danger" row icon="trash-2" label={t("invites.revoke")} busyLabel={t("invites.revoking")} />
      </form>
    </InPlaceConfirm>
  );
}

/** An invite (`IF2QM`): open ones can be revoked, confirmed in place (`KagFB`). */
export function InviteRow({ invite, csrf, server, now }: { invite: Invite; csrf: string; server: string; now: number }) {
  const [state, action] = useActionState<RevokeState, FormData>(revokeInviteAction, {});
  const [asking, setAsking] = useState(false);
  if (state.revoked !== undefined) {
    const text = state.revoked === "told" ? t("invites.revokedText", { server }) : t("invites.revokedCloudLater", { server });
    return <DoneRow title={t("invites.revokedTitle")} text={text} />;
  }
  const badge = STATES[invite.state];
  const side = (
    <span className={styles.side}>
      <span className={styles.state}>
        <Badge tone={badge.tone}>{t(badge.key)}</Badge>
      </span>
      <span className={styles.when}>
        <When invite={invite} now={now} />
      </span>
      <span className={styles.actions}>
        {invite.state === "open" && (
          <ActionButton kind="ghost" row expanded={asking} onClick={() => { setAsking(true); }}>
            {t("invites.revoke")}
          </ActionButton>
        )}
      </span>
    </span>
  );
  return (
    <>
      <Row glyph={invite.delivery === "code" ? "key-round" : "mail"} align="center" compact active={asking} side={side}>
        <TopLine name={title(invite)} weight="semibold">
          <Chip>{chip(invite)}</Chip>
        </TopLine>
        <RowLine>
          <Sent invite={invite} now={now} />
        </RowLine>
      </Row>
      {asking && <Revoke invite={invite} csrf={csrf} action={action} close={() => { setAsking(false); }} />}
      {state.error !== undefined && (
        <div className={styles.under}>
          <ErrorLine message={state.error} />
        </div>
      )}
    </>
  );
}
