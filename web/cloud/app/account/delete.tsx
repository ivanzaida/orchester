"use client";

import { useActionState, useState } from "react";

import { ActionButton } from "@orchester/ui/components/button";
import { RowLine, SettingRow } from "@orchester/ui/components/list";
import { Submit } from "@orchester/ui/components/submit";

import { ErrorLine, PasswordField } from "@/components/fields";
import { CsrfField } from "@/components/hidden";
import { t } from "@/lib/i18n";

import { deleteAccountAction, type DeleteState } from "./actions";
import { markSection } from "./nav";
import styles from "./account.module.css";

const CLOSED: DeleteState = { view: "closed" };

/**
 * *Delete account*, confirmed in place as the web's other destructive actions are (Server admin `QEHMk`): the
 * question, what happens, the account's password, *Keep* and *Delete account*. A deletion ends this browser's session
 * and leaves for sign-in.
 */
export function DeleteAccount({ csrf, email, owned }: { csrf: string; email: string; owned: readonly string[] | null }) {
  const [state, action] = useActionState<DeleteState, FormData>(deleteAccountAction, CLOSED);
  const [open, setOpen] = useState(false);
  const [seen, setSeen] = useState(state);
  const [password, setPassword] = useState("");
  if (seen !== state) {
    setSeen(state);
    setOpen(state.view === "open");
    setPassword("");
  }
  const openConfirmation = () => {
    setOpen(true);
    markSection("account");
  };
  return (
    <SettingRow label={t("account.settings.deleteAccount")}>
      {open ? (
        <form className={styles.confirmation} action={action} noValidate>
          <CsrfField token={csrf} />
          <p className={styles.confirmationTitle}>{t("account.settings.deleteTitle")}</p>
          <p className={styles.confirmationText}>{t("account.settings.deleteText", { email })}</p>
          <OwnedServers owned={owned} />
          <span className={styles.form}>
            <PasswordField name="password" size="row" label={t("account.settings.deletePassword")} value={password}
              onChange={setPassword} error={state.view === "open" ? state.errors.password : undefined}
              hint={t("account.settings.deletePasswordHint")} autoComplete="current-password" />
          </span>
          {state.view === "open" && state.error && <ErrorLine message={state.error} />}
          <span className={styles.buttons}>
            <ActionButton kind="secondary" row onClick={() => { setOpen(false); }}>
              {t("account.settings.keep")}
            </ActionButton>
            <Submit kind="danger" row icon="trash-2" label={t("account.settings.deleteAccount")}
              busyLabel={t("account.settings.deleting")} />
          </span>
        </form>
      ) : (
        <>
          <ActionButton kind="secondary" row icon="trash-2" onClick={openConfirmation}>
            {t("account.settings.deleteAccount")}
          </ActionButton>
          <RowLine>{t("account.settings.deleteHint")}</RowLine>
        </>
      )}
    </SettingRow>
  );
}

/**
 * The team servers this account linked: deleting it leaves them registered with no owner, their members' joins and
 * invites too, until an admin of each links it again. Without the list (it did not load), the line says so generally.
 * Either way the servers keep a keyed hash of the email for the per-person limits (`docs/cloud-api.md`, "Deleting the
 * account"), and a second line says so.
 */
function OwnedServers({ owned }: { owned: readonly string[] | null }) {
  if (owned !== null && owned.length === 0) {
    return null;
  }
  const servers = owned === null
    ? t("account.settings.deleteServersUnknown")
    : t("account.settings.deleteServers", { servers: owned.join(", ") });
  return (
    <>
      <p className={styles.confirmationText}>{servers}</p>
      <p className={styles.confirmationText}>{t("account.settings.deleteServersHash")}</p>
    </>
  );
}
