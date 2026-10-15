"use client";

import { useActionState, useState } from "react";

import { Checkbox } from "@orchester/ui/components/checkbox";
import { Submit } from "@orchester/ui/components/submit";

import { codeInviteAction, type CodeState } from "@/app/admin/shared/invite-actions";
import { ErrorLine } from "@/components/fields";
import { CsrfField, Hidden } from "@/components/hidden";
import { CodeCard } from "@/components/invites/code-card";
import { EmailInvite } from "@/components/invites/email-invite";
import { InviteCard } from "@/components/invites/invite-card";
import { t } from "@/lib/i18n";
import type { EmailReach } from "@/lib/model";

import styles from "@/components/invites/invites.module.css";

const START: CodeState = { view: "form", answer: 0 };

interface Props {
  readonly csrf: string;
  readonly server: string;
  readonly reach: EmailReach;
  readonly back: string;
  readonly address: string;
  readonly checkCode: string | null;
}

/**
 * *Invite to the server* (`PPiT4`): by email through the cloud, or *Invite with a code*, for an account with no
 * project yet; *As an admin* applies to both. The code is shown once under the form.
 */
export function ServerInvite(props: Props) {
  const [admin, setAdmin] = useState(false);
  const [code, codeAction] = useActionState<CodeState, FormData>(codeInviteAction, START);
  const [dropped, setDropped] = useState(0);
  const scope = { kind: "server", admin: admin ? "yes" : "no" };
  const asAdmin = (
    <span className={styles.checkbox}>
      <Checkbox name="adminChoice" label={t("people.asAdmin")} checked={admin} onChange={setAdmin} />
    </span>
  );
  const codeForm = (
    <form className={styles.button} action={codeAction}>
      <CsrfField token={props.csrf} />
      <Hidden name="kind" value="server" />
      <Hidden name="admin" value={admin ? "yes" : "no"} />
      <Submit kind="secondary" row icon="key-round" label={t("people.withCode")} busyLabel={t("people.making")} />
    </form>
  );
  const ready = props.reach.kind === "ready";
  return (
    <InviteCard id="invite-server" icon="user-plus" title={t("people.inviteTitle")} text={t("people.inviteText")}>
      <EmailInvite csrf={props.csrf} server={props.server} reach={props.reach} back={props.back} scope={scope}
        extra={ready ? asAdmin : null} beside={ready ? codeForm : undefined} sentText="people.sentText"
        sentTitle={(email, isAdmin) => (isAdmin ? t("people.sentTitleAdmin", { email }) : t("people.sentTitle", { email }))} />
      {!ready && (
        <div className={styles.form}>
          {asAdmin}
          {codeForm}
        </div>
      )}
      {code.view === "code" && dropped !== code.answer && (
        <CodeCard title={code.admin ? t("people.serverCodeCardAdmin", { server: props.server }) : t("people.serverCodeCard", { server: props.server })}
          code={code.code} address={props.address} checkCode={props.checkCode} expiresAt={code.expiresAt} hours={code.hours}
          warning={t("invites.codeWarning")} done={() => { setDropped(code.answer); }} />
      )}
      {code.view === "form" && code.error !== undefined && <ErrorLine message={code.error} />}
    </InviteCard>
  );
}
