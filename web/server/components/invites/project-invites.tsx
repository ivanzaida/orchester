"use client";

import { Select } from "@orchester/ui/components/select";

import { t } from "@/lib/i18n";

import { CodeInvite } from "./code-invite";
import type { EmailReach } from "@/lib/model";

import { EmailInvite } from "./email-invite";
import { InviteCard } from "./invite-card";
import styles from "./invites.module.css";

function RoleSelect({ id }: { id: string }) {
  return (
    <span className={styles.role}>
      <Select name="role" id={id} label={t("invites.role")} defaultValue="member"
        options={[{ value: "member", label: t("common.member") }, { value: "owner", label: t("common.owner") }]} />
    </span>
  );
}

/** A project's two ways to invite (`iLFy5`): by email through Orchester Cloud, or with a code shown once. */
export function ProjectInvites(props: {
  csrf: string;
  server: string;
  project: { id: string; name: string; address: string };
  reach: EmailReach;
  checkCode: string | null;
}) {
  const scope = { kind: "project", project: props.project.id };
  const words = { project: props.project.name, server: props.server };
  return (
    <>
      <InviteCard id="invite-email" icon="mail" title={t("invites.emailTitle")} text={t("invites.emailText")}>
        <EmailInvite csrf={props.csrf} server={props.server} reach={props.reach} scope={scope}
          extra={<RoleSelect id="email-role" />} sentTitle={(email) => t("invites.sentTitle", { email })} sentText="invites.sentText" />
      </InviteCard>
      <InviteCard id="invite-code" icon="key-round" title={t("invites.codeTitle")} text={t("invites.codeText", words)}>
        <CodeInvite csrf={props.csrf} scope={scope} fields={<RoleSelect id="code-role" />} kind="secondary"
          label={t("invites.make")} busy={t("invites.making")} address={props.project.address} checkCode={props.checkCode}
          warning={t("invites.codeWarning")}
          title={(made) => t("invites.codeCard", { project: props.project.name, role: t(made.role === "owner" ? "invites.roleOwner" : "invites.roleMember") })} />
      </InviteCard>
    </>
  );
}
