"use client";

import { startTransition, useActionState, useState, type SubmitEvent } from "react";

import { ActionButton } from "@orchester/ui/components/button";
import { Callout } from "@orchester/ui/components/callout";
import { InPlaceConfirm } from "@orchester/ui/components/confirm";
import { DoneRow } from "@orchester/ui/components/list";
import { Select } from "@orchester/ui/components/select";
import { Submit } from "@orchester/ui/components/submit";
import { Cell, HeadCell, TableHead, TableRow } from "@orchester/ui/components/table";

import { ErrorLine } from "@/components/fields";
import { CsrfField, Hidden } from "@/components/hidden";
import { Person } from "@/components/person";
import { Phrase } from "@/components/when";
import { t } from "@/lib/i18n";
import type { Member } from "@/lib/model";

import { memberAction, type MemberState } from "./actions";
import styles from "./members.module.css";

interface RowProps {
  readonly member: Member;
  readonly me: string;
  readonly project: { readonly id: string; readonly name: string };
  readonly server: string;
  readonly csrf: string;
  readonly now: number;
}

/**
 * The role select sends its form as soon as it changes; without script its *Change* button does. It stays the
 * browser's own select, and what the chosen role means is under it, as the role menu says it (`KagFB`).
 */
function RoleForm({ props, action }: { props: RowProps; action: (form: FormData) => void }) {
  const { member } = props;
  const [role, setRole] = useState<string>(member.role);
  // Sent in a transition, not as a form action, so React does not reset the form and show the old role after the answer.
  const submit = (event: SubmitEvent<HTMLFormElement>) => {
    event.preventDefault();
    const data = new FormData(event.currentTarget);
    startTransition(() => { action(data); });
  };
  const hint = `role-hint-${member.user.id}`;
  const words = { project: props.project.name };
  return (
    <div className={styles.roleCell}>
      <form action={action} onSubmit={submit} className={styles.roleForm}>
        <CsrfField token={props.csrf} />
        <Hidden name="op" value="role" />
        <Hidden name="project" value={props.project.id} />
        <Hidden name="user" value={member.user.id} />
        <Select name="role" size="row" hideLabel label={t("members.roleOf", { name: member.user.displayName })}
          value={role} describedBy={hint}
          options={[{ value: "owner", label: t("common.owner") }, { value: "member", label: t("common.member") }]}
          onChange={(event) => {
            setRole(event.currentTarget.value);
            event.currentTarget.form?.requestSubmit();
          }} />
        <noscript>
          <Submit kind="ghost" row label={t("members.change")} busyLabel={t("members.changing")} />
        </noscript>
      </form>
      <span id={hint} className={styles.roleHint}>
        {role === "owner" ? t("members.ownerMeans", words) : t("members.memberMeans", words)}
      </span>
    </div>
  );
}

/** *Remove …?* or *Leave …?*, asked in place under the row (`GsAoB`). */
function Confirm(props: { row: RowProps; self: boolean; action: (form: FormData) => void; close: () => void }) {
  const { row, self } = props;
  const words = { name: row.member.user.displayName, project: row.project.name, server: row.server };
  return (
    <InPlaceConfirm title={self ? t("members.leaveTitle", words) : t("members.removeTitle", words)}
      text={self ? t("members.leaveText", words) : t("members.removeText", words)}>
      <ActionButton kind="secondary" row autoFocus onClick={props.close}>
        {t("common.keep")}
      </ActionButton>
      <form action={props.action}>
        <CsrfField token={row.csrf} />
        <Hidden name="op" value="remove" />
        <Hidden name="project" value={row.project.id} />
        <Hidden name="user" value={row.member.user.id} />
        <Hidden name="self" value={self ? "yes" : "no"} />
        <Submit kind="danger" row icon={self ? "log-out" : "trash-2"} label={self ? t("members.leave") : t("members.remove")}
          busyLabel={self ? t("members.leaving") : t("members.removing")} />
      </form>
    </InPlaceConfirm>
  );
}

function Refusal({ state, self, words }: { state: MemberState; self: boolean; words: Record<string, string> }) {
  if (state.lastOwner !== true && state.error === undefined) {
    return null;
  }
  return (
    <div className={styles.under}>
      {state.lastOwner === true && (
        <Callout tone="danger" icon="shield-alert" role="alert" title={t("members.lastOwnerTitle", words)}
          body={self ? t("members.lastOwnerText") : t("members.lastOwnerOther", words)} />
      )}
      {state.error !== undefined && <ErrorLine message={state.error} />}
    </div>
  );
}

function MemberRow(props: RowProps) {
  const { member } = props;
  const [state, action] = useActionState<MemberState, FormData>(memberAction, {});
  const [asking, setAsking] = useState(false);
  const [seen, setSeen] = useState(state);
  if (seen !== state) {
    setSeen(state);
    setAsking(false);
  }
  const self = member.user.id === props.me;
  const words = { name: member.user.displayName, project: props.project.name, server: props.server };
  if (state.removed === true) {
    return <DoneRow title={t("members.removedTitle", words)} text={t("members.removedText", words)} />;
  }
  return (
    <>
      <TableRow columns={styles.columns} active={asking}>
        <Cell area={styles.person}>
          <Person name={member.user.displayName} username={member.username} self={self} />
        </Cell>
        <Cell area={styles.role}>
          <RoleForm props={props} action={action} />
        </Cell>
        <Cell area={styles.joined}>
          <Phrase k="members.joinedOn" dates={{ date: { at: member.addedAt, format: "date" } }} now={props.now} />
        </Cell>
        <Cell end area={styles.actions}>
          <ActionButton kind="ghost" row expanded={asking} onClick={() => { setAsking(true); }}>
            {self ? t("members.leave") : t("members.remove")}
          </ActionButton>
        </Cell>
      </TableRow>
      {asking && <Confirm row={props} self={self} action={action} close={() => { setAsking(false); }} />}
      <Refusal state={state} self={self} words={words} />
    </>
  );
}

/** Members (`Mhudf`; role menu, remove in place and the last owner in `KagFB`). */
export function MembersTable(props: { members: readonly Member[] } & Omit<RowProps, "member">) {
  const { members, ...row } = props;
  return (
    <div className={styles.table} role="table" aria-label={t("members.person")}>
      <TableHead columns={styles.columns}>
        <HeadCell area={styles.person}>{t("members.person")}</HeadCell>
        <HeadCell area={styles.role}>{t("members.role")}</HeadCell>
        <HeadCell area={styles.joined}>{t("members.joined")}</HeadCell>
        <HeadCell area={styles.actions} />
      </TableHead>
      {members.map((member) => (
        <MemberRow key={member.user.id} member={member} {...row} />
      ))}
    </div>
  );
}
