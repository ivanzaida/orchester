"use client";

import { useActionState, useEffect, useState } from "react";

import { t } from "@/lib/i18n";
import { ButtonLink } from "@orchester/ui/components/button";
import { Actions, Address, Card, CardForm, Head, Small, Stack, Text, Top } from "@orchester/ui/components/card";
import { ErrorLine, PasswordField, TextField } from "@/components/fields";
import { CsrfField, Hidden } from "@/components/hidden";
import { rememberSignInEmail } from "@/components/sign-in-email";
import { Submit } from "@orchester/ui/components/submit";

import { resendForAction, type ResendState } from "../signup/sent/actions";
import { confirmAction, type ConfirmState } from "./actions";

/**
 * *This link no longer works* with a field for a new link (`X2Cij0`), shared with the reset page's expired view
 * (`w4yqZz`), which sends its own kind of link.
 */
export function ExpiredLink(props: {
  csrf: string;
  title: string;
  description: string;
  send: (state: ResendState, form: FormData) => Promise<ResendState>;
  sendLabel: string;
  sendingLabel: string;
}) {
  const [state, action] = useActionState<ResendState, FormData>(props.send, {});
  const [email, setEmail] = useState("");
  return (
    <Card>
      <Head title={props.title} description={props.description} tile={{ icon: "link-2-off", tone: "warning" }} />
      <CardForm action={action}>
        <CsrfField token={props.csrf} />
        <TextField name="email" type="email" label={t("field.email")} value={email} onChange={setEmail}
          placeholder={t("field.emailPlaceholder")} error={state.error} autoComplete="email" />
        <Actions>
          <Submit kind="primary" icon="send" label={props.sendLabel} busyLabel={props.sendingLabel} />
          <ButtonLink kind="secondary" href="/signin">
            {t("confirm.signIn")}
          </ButtonLink>
        </Actions>
      </CardForm>
    </Card>
  );
}

function Confirmed({ email }: { email: string }) {
  useEffect(() => {
    rememberSignInEmail(email);
  }, [email]);
  return (
    <Card>
      <Top>
        <Head title={t("confirm.confirmedTitle")} description={t("confirm.confirmedDescription")}
          tile={{ icon: "circle-check", tone: "success" }} />
        <Address email={email} />
      </Top>
      <Text>{t("confirm.confirmedNext")}</Text>
      <Actions>
        <ButtonLink kind="secondary" icon="log-in" href="/signin">
          {t("confirm.signInWeb")}
        </ButtonLink>
      </Actions>
    </Card>
  );
}

type AskState = Extract<ConfirmState, { view: "confirm" }>;

/** *Send a new link* after a failed try (`CVRpO`), to the address shown. */
function SendNewLink({ sending }: { sending: boolean }) {
  return (
    <Submit kind="secondary" icon="send" form="resend-form" label={t("confirm.send")} busyLabel={t("confirm.sending")}
      pending={sending} />
  );
}

/** The password asked (`Bdcjx`) and the one error state (`CVRpO`): the field is cleared and never marked red. */
function Ask({ csrf, token, state, action, confirming }: {
  csrf: string;
  token: string;
  state: AskState;
  action: (form: FormData) => void;
  confirming: boolean;
}) {
  const [resent, resend, sending] = useActionState<ResendState, FormData>(resendForAction, {});
  const [password, setPassword] = useState("");
  return (
    <Card>
      <Top>
        <Head
          title={t("confirm.title")}
          description={t("confirm.description")}
          tile={{ icon: "mail-check", tone: "neutral" }}
        />
        <Address email={state.email} />
      </Top>
      <CardForm id="confirm-form" action={action}>
        <CsrfField token={csrf} />
        <Hidden name="token" value={token} />
        <Stack>
          <PasswordField name="password" label={t("confirm.password")} value={password} onChange={setPassword}
            hint={t("confirm.passwordHint")} autoComplete="current-password" />
          {state.error && <ErrorLine message={state.error} />}
        </Stack>
      </CardForm>
      <form id="resend-form" action={resend} hidden>
        <CsrfField token={csrf} />
        <Hidden name="email" value={state.email} />
      </form>
      <Small>{t("confirm.notYou")}</Small>
      <Actions>
        <Submit kind="primary" icon="check" form="confirm-form" label={t("confirm.submit")}
          busyLabel={t("confirm.submitting")} pending={confirming} />
        {state.failed && <SendNewLink sending={sending} />}
        {resent.error && <ErrorLine message={resent.error} />}
      </Actions>
    </Card>
  );
}

/** The confirmation landing (`/confirm/{token}`, the link the cloud emails): its views. */
export function ConfirmViews({ csrf, token, initial }: { csrf: string; token: string; initial: ConfirmState }) {
  const [state, action, confirming] = useActionState(confirmAction, initial);
  if (state.view === "confirmed") {
    return <Confirmed email={state.email} />;
  }
  if (state.view === "expired") {
    return (
      <ExpiredLink csrf={csrf} title={t("confirm.expiredTitle")} description={t("confirm.expiredDescription")}
        send={resendForAction} sendLabel={t("confirm.send")} sendingLabel={t("confirm.sending")} />
    );
  }
  // A new answer remounts the form, which empties the password, as on every other page.
  return <Ask key={state.answer} csrf={csrf} token={token} state={state} action={action} confirming={confirming} />;
}
