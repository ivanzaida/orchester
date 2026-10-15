"use client";

import { useState } from "react";

import { field } from "@/lib/form";
import { t } from "@/lib/i18n";
import { signUpErrors, type FieldErrors, type SignUpInput } from "@/lib/validation";
import { Actions, Card, CardForm, Fields, Head, Small } from "@orchester/ui/components/card";
import { ErrorLine, PasswordField, TextField } from "@/components/fields";
import { CsrfField } from "@/components/hidden";
import { Submit } from "@orchester/ui/components/submit";
import { useCheckedForm } from "@orchester/ui/hooks/use-form";

import { signUpAction, type SignUpState } from "./actions";

const EMPTY: SignUpState = { email: "", displayName: "", errors: {} };

function read(form: FormData): SignUpInput {
  return { email: field(form, "email"), displayName: field(form, "displayName"), password: field(form, "password") };
}

/** Sign up (`fO9y2`; errors `cGB64`, submitting `o1Zmar`, phone `Ahged`). */
export function SignUpForm({ csrf }: { csrf: string }) {
  const [checked, setChecked] = useState<FieldErrors<keyof SignUpInput> | null>(null);
  const { state, pending, form } = useCheckedForm(signUpAction, EMPTY, (data) => {
    const errors = signUpErrors(read(data));
    setChecked(errors);
    return Object.keys(errors).length === 0;
  });
  const [email, setEmail] = useState(state.email);
  const [displayName, setDisplayName] = useState(state.displayName);
  const [password, setPassword] = useState("");
  const errors = checked !== null && Object.keys(checked).length > 0 ? checked : state.errors;
  return (
    <Card>
      <Head title={t("signup.title")} description={t("signup.description")} />
      <CardForm {...form}>
        <CsrfField token={csrf} />
        <Fields>
          <TextField name="email" type="email" label={t("field.email")} value={email} onChange={setEmail}
            error={errors.email} autoComplete="email" />
          <TextField name="displayName" type="text" label={t("signup.displayName")} value={displayName}
            onChange={setDisplayName} error={errors.displayName} hint={t("signup.displayNameHint")} autoComplete="name" />
          <PasswordField name="password" label={t("field.password")} value={password} onChange={setPassword}
            error={errors.password} hint={t("field.passwordRule")} autoComplete="new-password" />
        </Fields>
        <Actions>
          {state.formError && <ErrorLine message={state.formError} />}
          <Submit kind="primary" label={t("signup.submit")} busyLabel={t("signup.submitting")} pending={pending} />
          <Small>{t("signup.small")}</Small>
        </Actions>
      </CardForm>
    </Card>
  );
}
