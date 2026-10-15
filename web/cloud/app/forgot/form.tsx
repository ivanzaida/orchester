"use client";

import { useState } from "react";

import { field } from "@/lib/form";
import { t, type Message } from "@/lib/i18n";
import { emailError } from "@/lib/validation";
import { Card, CardForm, Head } from "@/ui/card";
import { TextField } from "@/ui/field";
import { CsrfField } from "@/ui/hidden";
import { Submit } from "@/ui/submit";
import { useCheckedForm } from "@/ui/use-form";

import { forgotAction } from "./actions";

/** Forgot password (`TjysO`). */
export function ForgotForm({ csrf }: { csrf: string }) {
  const [checked, setChecked] = useState<Message | undefined>(undefined);
  const { state, pending, form } = useCheckedForm(forgotAction, {}, (data) => {
    const error = emailError(field(data, "email"));
    setChecked(error);
    return error === undefined;
  });
  const [email, setEmail] = useState(state.email ?? "");
  return (
    <Card>
      <Head title={t("forgot.title")} description={t("forgot.description")} />
      <CardForm {...form}>
        <CsrfField token={csrf} />
        <TextField name="email" type="email" label={t("field.email")} value={email} onChange={setEmail}
          error={checked ?? state.error} autoComplete="email" />
        <Submit kind="primary" icon="send" label={t("forgot.submit")} busyLabel={t("forgot.submitting")} pending={pending} />
      </CardForm>
    </Card>
  );
}
