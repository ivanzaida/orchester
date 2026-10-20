"use client";

import { useEffect, useState, type ReactNode } from "react";

import { ButtonLink } from "@orchester/ui/components/button";
import { Callout } from "@orchester/ui/components/callout";
import { Submit } from "@orchester/ui/components/submit";
import { useCheckedForm } from "@orchester/ui/hooks/use-form";

import { emailInviteAction, type EmailState } from "@/app/admin/shared/invite-actions";
import { TextField } from "@/components/fields";
import { CsrfField, Hidden } from "@/components/hidden";
import { Say } from "@/components/say";
import { Phrase } from "@/components/when";
import { t, type MessageKey } from "@/lib/i18n";
import type { CloudCause, EmailReach } from "@/lib/model";
import { twoFactorNotice } from "@/lib/two-factor";

import styles from "./invites.module.css";

/** *… can't reach Orchester Cloud*, by what went wrong, so whoever runs the host knows where to look. */
const BROWSER_TITLES: Readonly<Record<CloudCause, MessageKey>> = {
  unreachable: "invites.browserTitle",
  tls: "invites.browserTls",
  refused: "invites.browserRefused",
  timeout: "invites.browserTimeout",
};

type Ready = Extract<EmailReach, { kind: "ready" }>;

export interface EmailInviteProps {
  readonly csrf: string;
  readonly server: string;
  readonly reach: EmailReach;
  /** The hidden fields that say who the invite is for (`kind`, `project`). */
  readonly scope: Readonly<Record<string, string>>;
  /** The fields beside the email: the role select, or *As an admin*. */
  readonly extra: ReactNode;
  /** A second form beside *Send invite* on People: *Invite with a code*. It is a sibling, not inside this form. */
  readonly beside?: ReactNode;
  /** `sent` words: a project invite or one to the server. */
  readonly sentTitle: (email: string, admin: boolean) => string;
  readonly sentText: "invites.sentText" | "people.sentText";
  /** Each invite made: the form's other fields clear for the next one too (*As an admin*, `Gs9Vu`). Keep it stable. */
  readonly onSent?: () => void;
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
        <Submit kind="primary" row icon="send" label={t("invites.browserSend")} busyLabel={t("invites.sending")} />
      </form>
    );
    return (
      <Callout tone="warning" icon="wifi-off" title={t(BROWSER_TITLES[state.cause], { server: props.server })}
        body={t("invites.browserText", { email: state.email })} action={send} />
    );
  }
  if (state.view === "refused") {
    return <Callout tone="danger" icon="circle-alert" role="alert" title={t("invites.refusedTitle")} body={t("invites.refusedText", { email: state.email, server: props.server })} />;
  }
  if (state.view === "form" && state.limited !== undefined) {
    return <Callout tone="danger" icon="circle-alert" role="alert" title={t("invites.limitTitle")} body={<Say message={state.limited} />} />;
  }
  if (state.view === "form" && props.reach.kind === "ready") {
    return <TwoFactorNeeded reach={props.reach} csrf={props.csrf} refused={state.mfa === true} />;
  }
  return null;
}

/** Ends this sign-in and starts Orchester Cloud's in one step, back to this page (`/admin/trip`, `purpose=again`). */
function SignInAgain({ csrf, page }: { csrf: string; page: string }) {
  return (
    <form action="/admin/trip" method="post">
      <CsrfField token={csrf} />
      <Hidden name="purpose" value="again" />
      <Hidden name="page" value={page} />
      <Submit kind="primary" row icon="log-in" label={t("invites.mfaAgain")} busyLabel={t("invites.mfaAgainBusy")} />
    </form>
  );
}

/**
 * The server sends invites by email only for an admin signed in with two-factor authentication (`403 mfa_required`),
 * shown as soon as the page knows this sign-in was made without it. Turned on since, it takes a new sign-in: *Sign in
 * again*. Where to turn it on is offered only once a sign-in again came back without it. `refused`: the server just
 * answered `mfa_required`.
 */
function TwoFactorNeeded({ reach, csrf, refused }: { reach: Ready; csrf: string; refused: boolean }) {
  const notice = twoFactorNotice(reach.twoFactor, refused);
  if (notice === "none") {
    return null;
  }
  if (notice === "again") {
    return (
      <Callout tone="warning" icon="shield-alert" role={refused ? "alert" : "status"} title={t("invites.mfaAgainTitle")}
        body={t("invites.mfaAgainText")} action={<SignInAgain csrf={csrf} page={reach.page} />} />
    );
  }
  const actions = (
    <>
      <ButtonLink href={`${reach.cloud}/account#security`} kind="secondary" row icon="shield">
        {t("invites.mfaOpen")}
      </ButtonLink>
      <SignInAgain csrf={csrf} page={reach.page} />
    </>
  );
  return (
    <Callout tone="warning" icon="shield-alert" role={refused ? "alert" : "status"} title={t("invites.mfaTitle")}
      body={t("invites.mfaText")} action={actions} />
  );
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
  const made = state.view === "form" ? null : state.answer;
  const { onSent } = props;
  useEffect(() => {
    if (made !== null) {
      onSent?.();
    }
  }, [made, onSent]);
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
