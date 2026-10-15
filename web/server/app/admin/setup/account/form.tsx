"use client";

import { useState } from "react";

import { Fields } from "@orchester/ui/components/card";
import { Submit } from "@orchester/ui/components/submit";
import { useCheckedForm } from "@orchester/ui/hooks/use-form";

import { ErrorLine, TextField } from "@/components/fields";
import { CsrfField } from "@/components/hidden";
import { t } from "@/lib/i18n";

import { accountAction, type AccountState } from "./actions";
import styles from "../setup.module.css";

/** Setup step 2 (`y9I12Q`): the server's name, then the first admin's username and display name. */
export function AccountForm({ csrf, serverName }: { csrf: string; serverName: string }) {
  const start: AccountState = { attempt: 0, values: { serverName, username: "", displayName: "" }, errors: {} };
  const { state, pending, form } = useCheckedForm(accountAction, start);
  const [values, setValues] = useState(state.values);
  const [seen, setSeen] = useState(state.attempt);
  if (seen !== state.attempt) {
    setSeen(state.attempt);
    setValues(state.values);
  }
  const set = (name: keyof AccountState["values"]) => (value: string) => { setValues({ ...values, [name]: value }); };
  return (
    <form className={styles.form} {...form} noValidate>
      <CsrfField token={csrf} />
      <Fields>
        <TextField name="serverName" type="text" size="row" label={t("setup.serverName")} value={values.serverName}
          onChange={set("serverName")} hint={t("setup.serverNameHint")} error={state.errors.serverName} autoComplete="organization" />
        <TextField name="username" type="text" size="row" label={t("setup.username")} value={values.username}
          onChange={set("username")} hint={t("setup.usernameHint")} error={state.errors.username} autoComplete="username" mono />
        <TextField name="displayName" type="text" size="row" label={t("setup.displayName")} value={values.displayName}
          onChange={set("displayName")} error={state.errors.displayName} autoComplete="name" />
        {state.error && <ErrorLine message={state.error} />}
      </Fields>
      <Submit kind="primary" label={t("setup.continue")} busyLabel={t("setup.continuing")} pending={pending} />
    </form>
  );
}
