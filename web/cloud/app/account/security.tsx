"use client";

import { useRouter } from "next/navigation";
import { useActionState, useState } from "react";

import { ActionButton } from "@orchester/ui/components/button";
import { Note } from "@orchester/ui/components/card";
import { Badge, Panel, RowLine, Section, SettingRow } from "@orchester/ui/components/list";
import { Submit } from "@orchester/ui/components/submit";

import { ErrorLine, PasswordField, TextField } from "@/components/fields";
import { CsrfField } from "@/components/hidden";
import { OnSince } from "@/components/moment";
import { t } from "@/lib/i18n";
import { FEW_RECOVERY_CODES, type TwoFactorStatus } from "@/lib/two-factor";

import { markSection } from "./nav";
import {
  recoveryCodesAction,
  stepUpAction,
  turnOffAction,
  turnOnAction,
  type CodeFormState,
  type CodeStep,
  type OffState,
  type SetupState,
  type SetupStep,
} from "./security-actions";
import { CodesShown } from "./security-codes";
import { TurnOn } from "./security-setup";
import styles from "./security.module.css";

/** A code field and its submit on one line, with what the cloud said under them. */
function CodeLine(props: { csrf: string; action: (form: FormData) => void; error: CodeFormState; submit: [string, string] }) {
  const [code, setCode] = useState("");
  const error = props.error.view === "open" ? props.error.error : undefined;
  return (
    <form className={styles.line} action={props.action} noValidate>
      <CsrfField token={props.csrf} />
      <span className={styles.field}>
        <TextField name="code" type="text" size="row" label={t("account.security.anyCodeLabel")} hideLabel value={code}
          onChange={setCode} error={error} autoComplete="one-time-code" code />
      </span>
      <Submit kind="secondary" row label={props.submit[0]} busyLabel={props.submit[1]} unavailable={code.trim() === ""} />
    </form>
  );
}

/** *Turn off*, confirmed in place with the password and a code, as *Delete account* is. */
function TurnOff({ csrf }: { csrf: string }) {
  const [state, action] = useActionState<OffState, FormData>(turnOffAction, { view: "closed" });
  const [open, setOpen] = useState(false);
  const [seen, setSeen] = useState(state);
  const [password, setPassword] = useState("");
  const [code, setCode] = useState("");
  if (seen !== state) {
    setSeen(state);
    setOpen(state.view === "open");
    setPassword("");
    setCode("");
  }
  if (!open) {
    return (
      <ActionButton kind="secondary" row onClick={() => { setOpen(true); markSection("security"); }}>
        {t("account.security.turnOff")}
      </ActionButton>
    );
  }
  const errors = state.view === "open" ? state.errors : {};
  return (
    <form className={styles.flow} action={action} noValidate>
      <CsrfField token={csrf} />
      <p className={styles.title}>{t("account.security.offTitle")}</p>
      <p className={styles.text}>{t("account.security.offText")}</p>
      <span className={styles.form}>
        <PasswordField name="password" size="row" label={t("account.security.passwordLabel")} value={password}
          onChange={setPassword} error={errors.password} autoComplete="current-password" />
        <TextField name="code" type="text" size="row" label={t("account.security.anyCodeLabel")} value={code}
          onChange={setCode} error={errors.code} autoComplete="one-time-code" code />
      </span>
      {state.view === "open" && state.error && <ErrorLine message={state.error} />}
      <span className={styles.buttons}>
        <ActionButton kind="secondary" row onClick={() => { setOpen(false); }}>
          {t("account.security.keep")}
        </ActionButton>
        <Submit kind="danger" row label={t("account.security.turnOff")} busyLabel={t("account.security.turningOff")} />
      </span>
    </form>
  );
}

const SETUP_CLOSED: SetupState = { view: "closed" };
const CODES_CLOSED: CodeFormState = { view: "closed" };

/**
 * *Turn on*'s next answer from its last: the server takes back only the step (`SetupStep`), so the key and the
 * recovery codes are never posted again; the key stays shown when a wrong code answers without it.
 */
async function nextSetup(previous: SetupState, form: FormData): Promise<SetupState> {
  const step: SetupStep = previous.view === "password" || previous.view === "scan"
    ? { view: previous.view, answer: previous.answer }
    : { view: "closed" };
  const next = await turnOnAction(step, form);
  if (next.view === "scan" && next.key === undefined && previous.view === "scan") {
    return { ...next, key: previous.key };
  }
  return next;
}

/** A one-code form's next answer from its last, which gives the server only how many answers there were. */
function nextCodes(send: (previous: CodeStep, form: FormData) => Promise<CodeFormState>) {
  return (previous: CodeFormState, form: FormData): Promise<CodeFormState> =>
    send({ answer: previous.view === "open" ? previous.answer : 0 }, form);
}

const nextRecoveryCodes = nextCodes(recoveryCodesAction);
const nextStepUp = nextCodes(stepUpAction);

function TwoFactorSetting({ status, csrf }: { status: TwoFactorStatus; csrf: string }) {
  const label = t("account.security.twoFactor");
  const [setup, setupAction] = useActionState<SetupState, FormData>(nextSetup, SETUP_CLOSED);
  const [doneWith, setDoneWith] = useState<SetupState | null>(null);
  const router = useRouter();
  if (setup.view === "codes" && doneWith !== setup) {
    // Shown until *Done*, which only then reads the page again.
    const done = () => {
      setDoneWith(setup);
      router.refresh();
    };
    return (
      <SettingRow label={label}>
        <CodesShown codes={setup.codes} onDone={done} />
      </SettingRow>
    );
  }
  if (!status.enabled || status.enabledAt === null) {
    return (
      <SettingRow label={label}>
        <Badge tone="neutral">{t("account.security.off")}</Badge>
        <TurnOn csrf={csrf} state={setup} action={setupAction} />
      </SettingRow>
    );
  }
  return (
    <SettingRow label={label}>
      <Badge>{t("account.security.on")}</Badge>
      <RowLine>
        <OnSince at={status.enabledAt} />
      </RowLine>
      <TurnOff csrf={csrf} />
    </SettingRow>
  );
}

/** The recovery codes left, and *Make new codes* with a code, which shows the new ones once. */
function RecoverySetting({ left, csrf }: { left: number; csrf: string }) {
  const [state, action] = useActionState<CodeFormState, FormData>(nextRecoveryCodes, CODES_CLOSED);
  const [open, setOpen] = useState(false);
  const [seen, setSeen] = useState(state);
  if (seen !== state) {
    setSeen(state);
    setOpen(state.view !== "closed");
  }
  const count = left <= FEW_RECOVERY_CODES ? t("account.security.recoveryFew", { count: left }) : t("account.security.recoveryLeft", { count: left });
  const label = t("account.security.recoveryCodes");
  if (open && state.view === "codes") {
    return (
      <SettingRow label={label}>
        <CodesShown codes={state.codes} onDone={() => { setOpen(false); }} />
      </SettingRow>
    );
  }
  return (
    <SettingRow label={label}>
      <RowLine tone={left <= FEW_RECOVERY_CODES ? "secondary" : "muted"}>{count}</RowLine>
      {open ? (
        <>
          <RowLine>{t("account.security.newCodesHint")}</RowLine>
          <CodeLine key={state.view === "open" ? state.answer : 0} csrf={csrf} action={action} error={state}
            submit={[t("account.security.newCodes"), t("account.security.makingCodes")]} />
        </>
      ) : (
        <ActionButton kind="secondary" row icon="key-round" onClick={() => { setOpen(true); markSection("security"); }}>
          {t("account.security.newCodes")}
        </ActionButton>
      )}
    </SettingRow>
  );
}

/** A browser signed in before two-factor authentication was on, or without it: a code makes it a two-factor session. */
function ThisBrowser({ csrf }: { csrf: string }) {
  const [state, action] = useActionState<CodeFormState, FormData>(nextStepUp, CODES_CLOSED);
  return (
    <SettingRow label={t("account.security.thisBrowser")}>
      <RowLine>{t("account.security.thisBrowserText")}</RowLine>
      <CodeLine key={state.view === "open" ? state.answer : 0} csrf={csrf} action={action} error={state}
        submit={[t("account.security.verify"), t("account.security.verifying")]} />
    </SettingRow>
  );
}

/** *Security* (`docs/cloud-api.md`, "Two-factor authentication"): turning it on and off, recovery codes, this browser. */
export function Security({ status, csrf }: { status: TwoFactorStatus; csrf: string }) {
  return (
    <Section id="security" title={t("account.security.title")} description={t("account.security.description")}>
      <Panel>
        <TwoFactorSetting status={status} csrf={csrf} />
        {status.enabled && <RecoverySetting left={status.recoveryCodesLeft} csrf={csrf} />}
        {status.enabled && !status.thisSession && <ThisBrowser csrf={csrf} />}
      </Panel>
      {status.enabled && <Note icon="info">{t("account.security.lostPhone")}</Note>}
    </Section>
  );
}
