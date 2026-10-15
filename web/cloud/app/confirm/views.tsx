"use client";

import { useActionState, useEffect, useState } from "react";

import { t } from "@/lib/i18n";
import { ButtonLink } from "@orchester/ui/components/button";
import { Actions, Address, Card, CardForm, Head, Small, Text, Top } from "@orchester/ui/components/card";
import { ErrorLine, TextField } from "@/components/fields";
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

function Confirmed({ email }: { email: string | null }) {
  useEffect(() => {
    if (email !== null) {
      rememberSignInEmail(email);
    }
  }, [email]);
  const description = email === null ? t("confirm.confirmedDescriptionNoAddress") : t("confirm.confirmedDescription");
  return (
    <Card>
      <Top>
        <Head title={t("confirm.confirmedTitle")} description={description} tile={{ icon: "circle-check", tone: "success" }} />
        {email !== null && <Address email={email} />}
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

/** The confirmation landing (`/confirm/{token}`, the link the cloud emails): its three views. */
export function ConfirmViews({ csrf, token, initial }: { csrf: string; token: string; initial: ConfirmState }) {
  const [state, action] = useActionState(confirmAction, initial);
  if (state.view === "confirmed") {
    return <Confirmed email={state.email} />;
  }
  if (state.view === "expired") {
    return (
      <ExpiredLink csrf={csrf} title={t("confirm.expiredTitle")} description={t("confirm.expiredDescription")}
        send={resendForAction} sendLabel={t("confirm.send")} sendingLabel={t("confirm.sending")} />
    );
  }
  return (
    <Card>
      <Top>
        <Head
          title={t("confirm.title")}
          description={state.email === null ? t("confirm.descriptionNoAddress") : t("confirm.description")}
          tile={{ icon: "mail-check", tone: "neutral" }}
        />
        {state.email !== null && <Address email={state.email} />}
      </Top>
      <Small>{t("confirm.notYou")}</Small>
      <CardForm action={action}>
        <CsrfField token={csrf} />
        <Hidden name="token" value={token} />
        {state.error && <ErrorLine message={state.error} />}
        <Submit kind="primary" icon="check" label={t("confirm.submit")} busyLabel={t("confirm.submitting")} />
      </CardForm>
    </Card>
  );
}
