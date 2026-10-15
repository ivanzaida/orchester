"use client";

import { useActionState, useState, type ReactNode } from "react";

import { ActionButton } from "@orchester/ui/components/button";
import { Callout } from "@orchester/ui/components/callout";
import { InPlaceConfirm } from "@orchester/ui/components/confirm";
import { Badge } from "@orchester/ui/components/list";
import { Submit } from "@orchester/ui/components/submit";
import { Cell, HeadCell, TableHead, TableRow } from "@orchester/ui/components/table";

import { ErrorLine } from "@/components/fields";
import { CsrfField, Hidden } from "@/components/hidden";
import { CodeInvite } from "@/components/invites/code-invite";
import { Person } from "@/components/person";
import { t } from "@/lib/i18n";
import type { Person as Account } from "@/lib/model";

import { personAction, type PersonState } from "./actions";
import styles from "./people.module.css";

interface RowProps {
  readonly person: Account;
  readonly me: string;
  readonly server: string;
  readonly address: string;
  readonly checkCode: string | null;
  readonly csrf: string;
}

type Asking = "none" | "admin" | "disable" | "code";

/** The hidden fields of a row's forms: who, and what to do. */
function OpForm({ csrf, user, op, children }: { csrf: string; user: string; op: string; children: ReactNode }) {
  return (
    <>
      <CsrfField token={csrf} />
      <Hidden name="user" value={user} />
      <Hidden name="op" value={op} />
      {children}
    </>
  );
}

/** The role badge: *Admin* in blue, *Disabled* greyed (`cyUWO`). */
function RoleBadge({ person }: { person: Account }) {
  if (person.disabled) {
    return <Badge tone="neutral">{t("people.disabled")}</Badge>;
  }
  return person.admin ? <Badge tone="info">{t("people.admin")}</Badge> : null;
}

/** The actions a row offers: an account code for someone who is not an admin, admin, disable, enable. */
function Actions({ props, ask, action }: { props: RowProps; ask: (what: Asking) => void; action: (form: FormData) => void }) {
  const { person } = props;
  const self = person.id === props.me;
  if (person.disabled) {
    return (
      <form action={action}>
        <OpForm csrf={props.csrf} user={person.id} op="enable">
          <Submit kind="ghost" row label={t("people.enable")} busyLabel={t("people.working")} />
        </OpForm>
      </form>
    );
  }
  return (
    <>
      {!person.admin && (
        <ActionButton kind="ghost" row onClick={() => { ask("code"); }}>
          {t("people.accountCode")}
        </ActionButton>
      )}
      {!person.admin && (
        <ActionButton kind="ghost" row onClick={() => { ask("admin"); }}>
          {t("people.makeAdmin")}
        </ActionButton>
      )}
      {!self && (
        <ActionButton kind="ghost" row onClick={() => { ask("disable"); }}>
          {t("people.disable")}
        </ActionButton>
      )}
    </>
  );
}

function Confirm({ props, asking, action, close }: { props: RowProps; asking: "admin" | "disable"; action: (form: FormData) => void; close: () => void }) {
  const words = { name: props.person.displayName, server: props.server };
  const admin = asking === "admin";
  return (
    <InPlaceConfirm title={admin ? t("people.makeAdminTitle", words) : t("people.disableTitle", words)}
      text={admin ? t("people.makeAdminText") : t("people.disableText", words)}>
      <ActionButton kind="secondary" row onClick={close}>
        {admin ? t("people.keepAsIs") : t("common.keep")}
      </ActionButton>
      <form action={action}>
        <OpForm csrf={props.csrf} user={props.person.id} op={asking}>
          <Submit kind={admin ? "primary" : "danger"} row icon={admin ? "shield" : "user-x"}
            label={admin ? t("people.makeAdmin") : t("people.disable")} busyLabel={t("people.working")} />
        </OpForm>
      </form>
    </InPlaceConfirm>
  );
}

function Under({ props, asking, state, action, close }: {
  props: RowProps;
  asking: Asking;
  state: PersonState;
  action: (form: FormData) => void;
  close: () => void;
}) {
  const { person } = props;
  return (
    <>
      {(asking === "admin" || asking === "disable") && <Confirm props={props} asking={asking} action={action} close={close} />}
      {asking === "code" && (
        <div className={styles.under}>
          <CodeInvite csrf={props.csrf} scope={{ kind: "account", user: person.id }} kind="secondary" label={t("people.accountCode")}
            busy={t("people.making")} address={props.address} checkCode={props.checkCode}
            warning={t("people.codeWarning", { name: person.displayName })}
            title={() => t("people.codeCard", { username: person.username })} />
        </div>
      )}
      {state.lastAdmin === true && (
        <div className={styles.under}>
          <Callout tone="danger" icon="shield-alert" role="alert" title={t("people.lastAdminTitle", { server: props.server })}
            body={t("people.lastAdminText")} />
        </div>
      )}
      {state.error !== undefined && (
        <div className={styles.under}>
          <ErrorLine message={state.error} />
        </div>
      )}
    </>
  );
}

function PersonRow(props: RowProps) {
  const { person } = props;
  const [state, action] = useActionState<PersonState, FormData>(personAction, {});
  const [asking, setAsking] = useState<Asking>("none");
  const [seen, setSeen] = useState(state);
  if (seen !== state) {
    setSeen(state);
    setAsking("none");
  }
  return (
    <>
      <TableRow columns={styles.columns} active={asking !== "none"}>
        <Cell>
          <Person name={person.displayName} username={person.username} self={person.id === props.me} />
        </Cell>
        <Cell>{person.cloud ? t("people.viaCloud") : t("people.viaCode")}</Cell>
        <Cell>{person.projects}</Cell>
        <Cell>
          <RoleBadge person={person} />
        </Cell>
        <Cell end>
          <Actions props={props} ask={setAsking} action={action} />
        </Cell>
      </TableRow>
      <Under props={props} asking={asking} state={state} action={action} close={() => { setAsking("none"); }} />
    </>
  );
}

/** Accounts (`waaNU`; account code, disable, make admin and the last admin in `Gs9Vu`). */
export function PeopleTable(props: { people: readonly Account[] } & Omit<RowProps, "person">) {
  const { people, ...row } = props;
  return (
    <div className={styles.table} role="table" aria-label={t("people.accounts")}>
      <TableHead columns={styles.columns}>
        <HeadCell>{t("people.person")}</HeadCell>
        <HeadCell>{t("people.signsIn")}</HeadCell>
        <HeadCell>{t("people.projects")}</HeadCell>
        <HeadCell>{t("people.role")}</HeadCell>
        <HeadCell />
      </TableHead>
      {people.map((person) => (
        <PersonRow key={person.id} person={person} {...row} />
      ))}
    </div>
  );
}
