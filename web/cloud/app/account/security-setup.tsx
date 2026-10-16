"use client";

import { useState } from "react";

import { ActionButton } from "@orchester/ui/components/button";
import { CopyButton } from "@orchester/ui/components/copy";
import { RowLine } from "@orchester/ui/components/list";
import { Submit } from "@orchester/ui/components/submit";

import { PasswordField, TextField } from "@/components/fields";
import { CsrfField, Hidden } from "@/components/hidden";
import { t } from "@/lib/i18n";

import { markSection } from "./nav";
import type { SetupState } from "./security-actions";
import styles from "./security.module.css";

type Scan = Extract<SetupState, { view: "scan" }>;
type Step = { action: (form: FormData) => void; csrf: string; close: () => void };

/** The password that starts the setup; a new answer remounts it, which empties the field. */
function PasswordStep({ action, csrf, close, error }: Step & { error: Extract<SetupState, { view: "password" }>["error"] }) {
  const [password, setPassword] = useState("");
  return (
    <form className={styles.flow} action={action} noValidate>
      <CsrfField token={csrf} />
      <Hidden name="step" value="password" />
      <span className={styles.form}>
        <PasswordField name="password" size="row" label={t("account.security.passwordLabel")} value={password}
          onChange={setPassword} error={error} hint={t("account.security.passwordHint")} autoComplete="current-password" />
      </span>
      <span className={styles.buttons}>
        <ActionButton kind="ghost" row onClick={close}>
          {t("account.security.cancel")}
        </ActionButton>
        <Submit kind="primary" row label={t("account.security.continue")} busyLabel={t("account.security.continuing")} />
      </span>
    </form>
  );
}

/** The key in groups of four, as authenticator apps print it. */
function grouped(secret: string): string {
  return secret.match(/.{1,4}/g)?.join(" ") ?? secret;
}

/** The QR code and its key for the authenticator app, and the app's first code, which turns it on. */
function ScanStep({ action, csrf, close, scan }: Step & { scan: Scan }) {
  const [code, setCode] = useState("");
  const { qr } = scan;
  return (
    <form className={styles.flow} action={action} noValidate>
      <CsrfField token={csrf} />
      <Hidden name="step" value="code" />
      <p className={styles.title}>{t("account.security.scanTitle")}</p>
      <div className={styles.scan}>
        <svg className={styles.qr} viewBox={`0 0 ${qr.size} ${qr.size}`} role="img" shapeRendering="crispEdges"
          aria-label={t("account.security.qrLabel")}>
          <path d={qr.path} />
        </svg>
        <div className={styles.keyColumn}>
          <p className={styles.text}>{t("account.security.scanText")}</p>
          <output className={styles.key}>{grouped(scan.secret)}</output>
          <CopyButton value={scan.secret} label={t("account.security.copyKey")} copiedLabel={t("account.security.copiedKey")} />
        </div>
      </div>
      <span className={styles.form}>
        <TextField name="code" type="text" size="row" label={t("account.security.codeLabel")} value={code} onChange={setCode}
          error={scan.error} autoComplete="one-time-code" code />
      </span>
      <span className={styles.buttons}>
        <ActionButton kind="ghost" row onClick={close}>
          {t("account.security.cancel")}
        </ActionButton>
        <Submit kind="primary" row icon="shield" label={t("account.security.confirm")}
          busyLabel={t("account.security.confirming")} />
      </span>
    </form>
  );
}

/**
 * *Turn on*, in place: the password, then the QR code and the app's code. Its state lives in the section, which shows
 * the recovery codes the last step answers whether or not the page already reads two-factor authentication as on.
 */
export function TurnOn({ csrf, state, action }: { csrf: string; state: SetupState; action: (form: FormData) => void }) {
  const [open, setOpen] = useState(state.view === "password" || state.view === "scan");
  const [seen, setSeen] = useState(state);
  if (seen !== state) {
    setSeen(state);
    setOpen(state.view === "password" || state.view === "scan");
  }
  const close = () => {
    setOpen(false);
  };
  if (!open) {
    return (
      <>
        <ActionButton kind="secondary" row icon="shield" onClick={() => { setOpen(true); markSection("security"); }}>
          {t("account.security.turnOn")}
        </ActionButton>
        <RowLine>{t("account.security.offHint")}</RowLine>
      </>
    );
  }
  if (state.view === "scan") {
    return <ScanStep key={state.answer} action={action} csrf={csrf} close={close} scan={state} />;
  }
  const answer = state.view === "password" ? state.answer : 0;
  return <PasswordStep key={answer} action={action} csrf={csrf} close={close}
    error={state.view === "password" ? state.error : undefined} />;
}
