"use client";

import { useState, type ReactNode } from "react";

import { Callout } from "@orchester/ui/components/callout";
import { Submit } from "@orchester/ui/components/submit";
import { useCheckedForm } from "@orchester/ui/hooks/use-form";

import { emailInviteAction, type EmailState } from "@/app/admin/shared/invite-actions";
import { TextField } from "@/components/fields";
import { CsrfField, Hidden } from "@/components/hidden";
import { Say } from "@/components/say";
import { Phrase } from "@/components/when";
import { t } from "@/lib/i18n";
import type { EmailReach } from "@/lib/model";

import styles from "./invites.module.css";


export interface EmailInviteProps {
  readonly csrf: string;
  readonly server: string;
  readonly reach: EmailReach;
  /** This page's own address, for the cloud's *back to the admin* link after an invite sent through the browser. */
  readonly back: string;
  /** The hidden fields that say who the invite is for (`kind`, `project`). */
  readonly scope: Readonly<Record<string, string>>;
  /** The fields beside the email: the role select, or *As an admin*. */
  readonly extra: ReactNode;
  /** A second form beside *Send invite* on People: *Invite with a code*. It is a sibling, not inside this form. */
  readonly beside?: ReactNode;
  /** `sent` words: a project invite or one to the server. */
  readonly sentTitle: (email: string, admin: boolean) => string;
  readonly sentText: "invites.sentText" | "people.sentText";
}

/** What happened to the last invite, above the form (`KagFB`: sent, sent through the browser, daily limit). */
function Answer({ state, props }: { state: EmailState; props: EmailInviteProps }) {
  if (state.view === "sent") {
    const when = { when: { at: state.expiresAt, format: "dateTime" as const } };
    return (
      <Callout tone="success" icon="send" title={props.sentTitle(state.email, state.admin)}
        body={<Phrase k={props.sentText} params={{ server: props.server }} dates={when} />} />
    );
  }
  if (state.view === "browser" && props.reach.kind === "ready") {
    const send = (
      <form action={`${props.reach.cloud}/invites/deliver`} method="post">
        <Hidden name="invite" value={state.signed} />
        <Hidden name="return" value={props.back} />
        <Submit kind="primary" row icon="send" label={t("invites.browserSend")} busyLabel={t("invites.sending")} />
      </form>
    );
    return (
      <Callout tone="warning" icon="wifi-off" title={t("invites.browserTitle", { server: props.server })}
        body={t("invites.browserText", { email: state.email })} action={send} />
    );
  }
  if (state.view === "refused") {
    return <Callout tone="danger" icon="circle-alert" role="alert" title={t("invites.refusedTitle")} body={t("invites.refusedText", { email: state.email, server: props.server })} />;
  }
  if (state.view === "form" && state.limited !== undefined) {
    return <Callout tone="danger" icon="circle-alert" role="alert" title={t("invites.limitTitle")} body={<Say message={state.limited} />} />;
  }
  return null;
}

function Unavailable({ reach, server }: { reach: EmailReach; server: string }) {
  if (reach.kind === "no_cloud") {
    return <Callout tone="neutral" icon="cloud-off" title={t("invites.noCloudTitle")} body={t("invites.noCloudText")} />;
  }
  return <Callout tone="neutral" icon="cloud-off" title={t("invites.unlinkedTitle")} body={t("invites.unlinkedText", { server })} />;
}

const START: EmailState = { view: "form", answer: 0, email: "" };

/** *Invite by email* (`vzkko`), shared by a project's Invites and People's *Invite to the server* (`PPiT4`). */
export function EmailInvite(props: EmailInviteProps) {
  const { state, pending, form } = useCheckedForm(emailInviteAction, START);
  const [email, setEmail] = useState("");
  const [seen, setSeen] = useState(state.answer);
  if (seen !== state.answer) {
    setSeen(state.answer);
    // The form clears for the next one once an invite is made (`KagFB`, *Invite by email · sent*).
    setEmail(state.view === "form" ? state.email : "");
  }
  if (props.reach.kind !== "ready") {
    return <Unavailable reach={props.reach} server={props.server} />;
  }
  const error = state.view === "form" ? state.error : undefined;
  return (
    <>
      <Answer state={state} props={props} />
      <div className={styles.form}>
        <form className={styles.contents} {...form} noValidate>
          <CsrfField token={props.csrf} />
          {Object.entries(props.scope).map(([name, value]) => <Hidden key={name} name={name} value={value} />)}
          <span className={styles.grow}>
            <TextField name="email" type="email" size="row" label={t("invites.email")} value={email} onChange={setEmail}
              placeholder={t("invites.emailPlaceholder")} error={error} autoComplete="off" />
          </span>
          {props.extra}
          <span className={styles.button}>
            <Submit kind="primary" row icon="send" label={t("invites.send")} busyLabel={t("invites.sending")} pending={pending}
              unavailable={state.view === "form" && state.limited !== undefined} />
          </span>
        </form>
        {props.beside}
      </div>
    </>
  );
}
