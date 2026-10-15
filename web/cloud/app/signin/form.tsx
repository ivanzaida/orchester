"use client";

import { useActionState, useState } from "react";

import { t } from "@/lib/i18n";
import { Actions, Card, CardForm, Fields, Head } from "@/ui/card";
import { useRemaining } from "@/ui/countdown";
import { AsideLink, PasswordField, TextField } from "@/ui/field";
import { CsrfField, Hidden } from "@/ui/hidden";
import { Say } from "@/ui/say";
import { ErrorLine, Notice, Reason } from "@/ui/status";
import { Submit } from "@/ui/submit";
import { useCheckedForm } from "@/ui/use-form";

import { resendForAction, type ResendState } from "../signup/sent/actions";
import { signInAction, type SignInState } from "./actions";

const START: SignInState = { email: "", attempt: 0, outcome: { kind: "none" } };

/** *Confirm your email first* with *Resend confirmation* (`I6ffG`), its own form above the sign-in form. */
function Unconfirmed({ csrf, email }: { csrf: string; email: string }) {
  const [state, action] = useActionState<ResendState, FormData>(resendForAction, {});
  const resend = (
    <form action={action}>
      <CsrfField token={csrf} />
      <Hidden name="email" value={email} />
      <Submit kind="secondary" small icon="refresh-cw" label={t("signin.resend")} busyLabel={t("signin.resending")} />
    </form>
  );
  return (
    <>
      <Notice title={t("signin.unconfirmedTitle")} body={t("signin.unconfirmedBody", { email })} action={resend} />
      {state.error && <ErrorLine message={state.error} />}
    </>
  );
}

/** Sign in (`Otw5d`; wrong `xzr0I`, unconfirmed `I6ffG`, too many `o4khR`, light `j7rxg`, phone `afqip`). */
export function SignInForm({ csrf }: { csrf: string }) {
  const { state, pending, form } = useCheckedForm(signInAction, START);
  const [email, setEmail] = useState(state.email);
  const [password, setPassword] = useState("");
  const [seen, setSeen] = useState(state.attempt);
  if (seen !== state.attempt) {
    setSeen(state.attempt);
    if (state.outcome.kind === "wrong") {
      setPassword("");
    }
  }
  const outcome = state.outcome;
  const wait = useRemaining(outcome.kind === "limited" ? outcome.ms : 0);
  const limited = outcome.kind === "limited" && wait > 0;
  const forgot = <AsideLink href="/forgot">{t("signin.forgot")}</AsideLink>;
  return (
    <Card>
      <Head title={t("signin.title")} description={t("signin.description")} />
      {outcome.kind === "unconfirmed" && <Unconfirmed csrf={csrf} email={outcome.email} />}
      <CardForm {...form}>
        <CsrfField token={csrf} />
        <Fields>
          <TextField name="email" type="email" label={t("field.email")} value={email} onChange={setEmail} autoComplete="email" />
          <PasswordField name="password" label={t("field.password")} value={password} onChange={setPassword}
            aside={forgot} autoComplete="current-password" />
          {outcome.kind === "wrong" && <ErrorLine message={{ key: "signin.wrong" }} />}
          {(outcome.kind === "limited" || outcome.kind === "failed") && <ErrorLine message={outcome.error} />}
        </Fields>
        <Actions>
          <Submit kind="primary" icon="log-in" label={t("signin.submit")} busyLabel={t("signin.submitting")}
            pending={pending} unavailable={limited} describedBy={limited ? "signin-reason" : undefined} />
          {limited && (
            <Reason id="signin-reason">
              <Say message={outcome.reason} />
            </Reason>
          )}
        </Actions>
      </CardForm>
    </Card>
  );
}
