"use client";

import { useActionState, useState, type ReactNode } from "react";

import { Submit } from "@orchester/ui/components/submit";

import { codeInviteAction, type CodeState } from "@/app/admin/shared/invite-actions";
import { ErrorLine } from "@/components/fields";
import { CsrfField, Hidden } from "@/components/hidden";

import { CodeCard } from "./code-card";
import styles from "./invites.module.css";

export interface CodeInviteProps {
  readonly csrf: string;
  readonly scope: Readonly<Record<string, string>>;
  /** What the code is for, from what the server answered: *Invite code for Kontur · member*. */
  readonly title: (made: Extract<CodeState, { view: "code" }>) => string;
  readonly warning: string;
  readonly address: string;
  readonly checkCode: string | null;
  /** The fields before the button, e.g. the role select. */
  readonly fields?: ReactNode;
  readonly label: string;
  readonly busy: string;
  readonly kind: "primary" | "secondary" | "ghost";
}

const START: CodeState = { view: "form", answer: 0 };

/**
 * A form that makes a code and then shows it once in place (`bdPlP`): *Make a code* on a project's Invites, *Invite
 * with a code* and *Account code* on People. *Done, I've given it* drops the code and brings the form back.
 */
export function CodeInvite(props: CodeInviteProps) {
  const [state, action] = useActionState<CodeState, FormData>(codeInviteAction, START);
  const [dropped, setDropped] = useState(0);
  if (state.view === "code" && dropped !== state.answer) {
    return (
      <CodeCard title={props.title(state)} code={state.code} address={props.address} checkCode={props.checkCode}
        expiresAt={state.expiresAt} hours={state.hours} warning={props.warning} done={() => { setDropped(state.answer); }} />
    );
  }
  return (
    <form className={styles.form} action={action}>
      <CsrfField token={props.csrf} />
      {Object.entries(props.scope).map(([name, value]) => <Hidden key={name} name={name} value={value} />)}
      {props.fields}
      <span className={props.fields === undefined ? undefined : styles.button}>
        <Submit kind={props.kind} row icon="key-round" label={props.label} busyLabel={props.busy} />
      </span>
      {state.view === "form" && state.error !== undefined && <ErrorLine message={state.error} />}
    </form>
  );
}
