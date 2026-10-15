"use client";

import { useState } from "react";

import { field } from "@/lib/form";
import { t } from "@/lib/i18n";
import { newPasswordErrors, type FieldErrors, type NewPasswordInput } from "@/lib/validation";
import { ButtonLink } from "@/ui/button";
import { Card, CardForm, Fields, Head, Note, Small } from "@/ui/card";
import { PasswordField } from "@/ui/field";
import { CsrfField, Hidden } from "@/ui/hidden";
import { ErrorLine } from "@/ui/status";
import { Submit } from "@/ui/submit";
import { useCheckedForm } from "@/ui/use-form";

import { ExpiredLink } from "../confirm/views";
import { forgotAction } from "../forgot/actions";
import { resetAction, type ResetState } from "./actions";

function Done() {
  return (
    <Card>
      <Head title={t("reset.doneTitle")} description={t("reset.doneDescription")} tile={{ icon: "circle-check", tone: "success" }} />
      <ButtonLink kind="primary" icon="log-in" href="/signin">
        {t("reset.signIn")}
      </ButtonLink>
    </Card>
  );
}

/** The reset link's landing (`/reset/{token}`, the link the cloud emails): its three views. */
export function ResetViews({ csrf, token, initial }: { csrf: string; token: string; initial: ResetState }) {
  const [checked, setChecked] = useState<FieldErrors<keyof NewPasswordInput> | null>(null);
  const { state, pending, form } = useCheckedForm(resetAction, initial, (data) => {
    const errors = newPasswordErrors({ password: field(data, "password"), repeat: field(data, "repeat") });
    setChecked(errors);
    return Object.keys(errors).length === 0;
  });
  const [password, setPassword] = useState("");
  const [repeat, setRepeat] = useState("");
  if (state.view === "done") {
    return <Done />;
  }
  if (state.view === "expired") {
    return (
      <ExpiredLink csrf={csrf} title={t("reset.expiredTitle")} description={t("reset.expiredDescription")}
        send={forgotAction} sendLabel={t("reset.send")} sendingLabel={t("reset.sending")} />
    );
  }
  const errors = checked !== null && Object.keys(checked).length > 0 ? checked : state.errors;
  return (
    <Card>
      <Head title={t("reset.title")} description={t("reset.description", { email: state.email })} />
      <CardForm {...form}>
        <CsrfField token={csrf} />
        <Hidden name="token" value={token} />
        <Fields>
          <PasswordField name="password" label={t("reset.newPassword")} value={password} onChange={setPassword}
            error={errors.password} autoComplete="new-password" />
          <PasswordField name="repeat" label={t("reset.repeat")} value={repeat} onChange={setRepeat}
            error={errors.repeat} autoComplete="new-password" />
          <Small>{t("field.passwordRule")}</Small>
        </Fields>
        <Note icon="monitor-smartphone">{t("reset.note")}</Note>
        {state.error && <ErrorLine message={state.error} />}
        <Submit kind="primary" icon="check" label={t("reset.submit")} busyLabel={t("reset.submitting")} pending={pending} />
      </CardForm>
    </Card>
  );
}
