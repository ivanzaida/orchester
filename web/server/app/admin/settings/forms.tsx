"use client";

import { useActionState, useState } from "react";

import { ActionButton } from "@orchester/ui/components/button";
import { InPlaceConfirm } from "@orchester/ui/components/confirm";
import { DoneLine } from "@orchester/ui/components/status";
import { Submit } from "@orchester/ui/components/submit";
import { SwitchButton } from "@orchester/ui/components/switch";

import { ErrorLine, TextField } from "@/components/fields";
import { CsrfField, Hidden } from "@/components/hidden";
import { t } from "@/lib/i18n";

import { settingAction, type SettingState } from "./actions";
import styles from "./settings.module.css";

const START: SettingState = { answer: 0 };

/** A setting's switch (`ICKGB`): pressing it sends the form that flips it. */
export function SwitchForm(props: { csrf: string; op: "joinApproval" | "accepting"; on: boolean; label: string; onWord: string; offWord: string }) {
  const [state, action, pending] = useActionState<SettingState, FormData>(settingAction, START);
  return (
    <form action={action}>
      <CsrfField token={props.csrf} />
      <Hidden name="op" value={props.op} />
      <Hidden name="value" value={props.on ? "off" : "on"} />
      <SwitchButton on={props.on} label={props.label} state={props.on ? props.onWord : props.offWord} busy={pending} />
      {state.error !== undefined && <ErrorLine message={state.error} />}
    </form>
  );
}

type Open = "none" | "pin" | "unlink";

function PinForm({ csrf, action, close, state }: { csrf: string; action: (form: FormData) => void; close: () => void; state: SettingState }) {
  const [key, setKey] = useState("");
  return (
    <form className={styles.pin} action={action} noValidate>
      <CsrfField token={csrf} />
      <Hidden name="op" value="pin" />
      <TextField name="key" type="text" size="row" label={t("settings.pinField")} value={key} onChange={setKey} hint={t("settings.pinHint")}
        error={state.error} autoComplete="off" mono />
      <div className={styles.buttons}>
        <ActionButton kind="ghost" row onClick={close}>
          {t("common.cancel")}
        </ActionButton>
        <Submit kind="primary" row icon="key-round" label={t("settings.pinSubmit")} busyLabel={t("settings.pinning")} />
      </div>
    </form>
  );
}

/** *Pin another key…* (a field in place) and *Unlink…* (a question in place) under the cloud key (`WV4AT`). */
export function CloudKeyActions({ csrf, server }: { csrf: string; server: string }) {
  const [state, action] = useActionState<SettingState, FormData>(settingAction, START);
  const [open, setOpen] = useState<Open>("none");
  const [seen, setSeen] = useState(state.answer);
  if (seen !== state.answer) {
    setSeen(state.answer);
    if (state.done !== undefined) {
      setOpen("none");
    }
  }
  if (open === "pin") {
    return <PinForm csrf={csrf} action={action} close={() => { setOpen("none"); }} state={state} />;
  }
  return (
    <>
      <div className={styles.buttons}>
        <ActionButton kind="secondary" row onClick={() => { setOpen("pin"); }}>
          {t("settings.pin")}
        </ActionButton>
        <ActionButton kind="ghost" row icon="unlink" onClick={() => { setOpen("unlink"); }}>
          {t("settings.unlink")}
        </ActionButton>
      </div>
      {state.done === "pinned" && <DoneLine>{t("settings.pinned")}</DoneLine>}
      {open === "unlink" && (
        <div className={styles.confirm}>
          <InPlaceConfirm title={t("settings.unlinkTitle", { server })} text={t("settings.unlinkText")}>
            <ActionButton kind="secondary" row onClick={() => { setOpen("none"); }}>
              {t("settings.keepLink")}
            </ActionButton>
            <form action={action}>
              <CsrfField token={csrf} />
              <Hidden name="op" value="unlink" />
              <Submit kind="danger" row icon="unlink" label={t("settings.unlinkSubmit")} busyLabel={t("settings.unlinking")} />
            </form>
          </InPlaceConfirm>
        </div>
      )}
      {state.error !== undefined && <ErrorLine message={state.error} />}
    </>
  );
}
