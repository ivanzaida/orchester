"use client";

import { useActionState, useState } from "react";

import { ActionButton } from "@orchester/ui/components/button";
import { Note } from "@orchester/ui/components/card";
import { Badge, Panel, RowLine, Section, SettingRow } from "@orchester/ui/components/list";
import { DoneLine } from "@orchester/ui/components/status";
import { Submit } from "@orchester/ui/components/submit";
import { useCheckedForm } from "@orchester/ui/hooks/use-form";

import { ErrorLine, PasswordField, TextField } from "@/components/fields";
import { CsrfField } from "@/components/hidden";
import { field } from "@/lib/form";
import { t } from "@/lib/i18n";
import { displayNameError } from "@/lib/validation";

import { passwordAction, renameAction, type FormState, type PasswordState } from "./actions";
import { DeleteAccount } from "./delete";
import { SignOutForm } from "./header";
import { markSection } from "./nav";
import styles from "./account.module.css";

/** The display name and *Save*, dimmed until the name changes (`szIXb`). */
function DisplayName({ name, csrf }: { name: string; csrf: string }) {
  const [checked, setChecked] = useState<FormState>({});
  const { state, pending, form } = useCheckedForm<FormState>(renameAction, {}, (data) => {
    const error = displayNameError(field(data, "displayName"));
    setChecked(error === undefined ? {} : { error });
    return error === undefined;
  });
  const [value, setValue] = useState(name);
  const error = checked.error ?? state.error;
  return (
    <SettingRow label={t("account.settings.displayName")}>
      <form className={styles.fieldLine} {...form} noValidate>
        <CsrfField token={csrf} />
        <span className={styles.field}>
          <TextField name="displayName" type="text" size="row" label={t("account.settings.displayName")} hideLabel
            value={value} onChange={setValue} error={error} autoComplete="name" />
        </span>
        <Submit kind="secondary" row label={t("account.settings.save")} busyLabel={t("account.settings.saving")}
          pending={pending} unavailable={value.trim() === name} />
      </form>
      <RowLine>{t("account.settings.displayNameHint")}</RowLine>
    </SettingRow>
  );
}

type OpenState = Extract<PasswordState, { view: "open" }>;

/** The open form: current, new and repeated password, the rule, the note, *Cancel* and *Change password*. */
function PasswordForm(props: { csrf: string; state: OpenState; action: (form: FormData) => void; close: () => void }) {
  const { state } = props;
  const [current, setCurrent] = useState("");
  const [next, setNext] = useState("");
  const [repeat, setRepeat] = useState("");
  return (
    <form className={styles.passwordForm} action={props.action} noValidate>
      <CsrfField token={props.csrf} />
      <span className={styles.form}>
        <PasswordField name="current" size="row" label={t("account.settings.currentPassword")} value={current}
          onChange={setCurrent} error={state.errors.current} autoComplete="current-password" />
        <PasswordField name="password" size="row" label={t("account.settings.newPassword")} value={next}
          onChange={setNext} error={state.errors.password} autoComplete="new-password" />
        <PasswordField name="repeat" size="row" label={t("account.settings.repeatPassword")} value={repeat}
          onChange={setRepeat} error={state.errors.repeat} autoComplete="new-password" />
      </span>
      <RowLine>{t("account.settings.newPasswordRule")}</RowLine>
      <Note icon="info">{t("account.settings.passwordNote")}</Note>
      {state.error && <ErrorLine message={state.error} />}
      <span className={styles.buttons}>
        <ActionButton kind="ghost" row onClick={props.close}>
          {t("account.settings.cancel")}
        </ActionButton>
        <Submit kind="primary" row icon="check" label={t("account.settings.changePassword")}
          busyLabel={t("account.settings.changingPassword")} />
      </span>
    </form>
  );
}

const CLOSED: PasswordState = { view: "closed", changed: false };
const FRESH: OpenState = { view: "open", answer: 0, errors: {} };

/** *Change password*, opened in place (`l5E5kC`, `s6SCbg`); a change closes it with a success line. */
function Password({ csrf }: { csrf: string }) {
  const [state, action] = useActionState<PasswordState, FormData>(passwordAction, CLOSED);
  const [open, setOpen] = useState(false);
  const [seen, setSeen] = useState(state);
  if (seen !== state) {
    setSeen(state);
    setOpen(state.view === "open");
  }
  const openForm = () => {
    setOpen(true);
    markSection("account");
  };
  if (open) {
    const shown = state.view === "open" ? state : FRESH;
    return (
      <SettingRow label={t("account.settings.password")}>
        {/* A new answer remounts the form, which empties its three fields, as on the other pages (`ZyAsL`). */}
        <PasswordForm key={shown.answer} csrf={csrf} state={shown} action={action}
          close={() => { setOpen(false); }} />
      </SettingRow>
    );
  }
  return (
    <SettingRow label={t("account.settings.password")}>
      <ActionButton kind="secondary" row icon="key-round" onClick={openForm}>
        {t("account.settings.changePassword")}
      </ActionButton>
      {state.view === "closed" && state.changed ? (
        <DoneLine>{t("account.settings.passwordChanged")}</DoneLine>
      ) : (
        <RowLine>{t("account.settings.passwordHint")}</RowLine>
      )}
    </SettingRow>
  );
}

export function Settings(props: { name: string; email: string; csrf: string; owned: readonly string[] | null }) {
  const { name, email, csrf } = props;
  return (
    <Section id="account" title={t("account.settings.title")}>
      <Panel>
        <DisplayName name={name} csrf={csrf} />
        <SettingRow label={t("account.settings.email")}>
          <span className={styles.emailLine}>
            <span className={styles.address}>{email}</span>
            <Badge>{t("account.settings.confirmed")}</Badge>
          </span>
          <RowLine>{t("account.settings.emailHint")}</RowLine>
        </SettingRow>
        <Password csrf={csrf} />
        <SettingRow label={t("account.settings.signOut")}>
          <SignOutForm csrf={csrf} kind="secondary" label={t("account.settings.signOutBrowser")} />
          <RowLine>{t("account.settings.signOutHint")}</RowLine>
        </SettingRow>
        <DeleteAccount csrf={csrf} email={email} owned={props.owned} />
      </Panel>
    </Section>
  );
}
