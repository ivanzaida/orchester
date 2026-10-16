"use client";

import { useActionState, useState } from "react";

import { DoneLine } from "@orchester/ui/components/status";
import { Submit } from "@orchester/ui/components/submit";

import { TextField } from "@/components/fields";
import { CsrfField, Hidden } from "@/components/hidden";
import { t } from "@/lib/i18n";

import { saveProjectAction, type SaveState } from "./actions";
import styles from "./settings.module.css";

const START: SaveState = { answer: 0, saved: false };

/** A setting's field and *Save*, dimmed until the value changes (`h8OLW5`, Name and Repository hint). */
export function SaveField(props: { csrf: string; project: string; which: "name" | "hint"; label: string; initial: string }) {
  const [state, action] = useActionState<SaveState, FormData>(saveProjectAction, START);
  const [value, setValue] = useState(props.initial);
  return (
    <form className={styles.line} action={action} noValidate>
      <CsrfField token={props.csrf} />
      <Hidden name="project" value={props.project} />
      <Hidden name="which" value={props.which} />
      <span className={styles.field}>
        <TextField name="value" type="text" size="row" label={props.label} hideLabel value={value} onChange={setValue}
          error={state.error} autoComplete="off" />
      </span>
      <Submit kind="secondary" row label={t("common.save")} busyLabel={t("common.saving")} unavailable={value.trim() === props.initial} />
      {state.saved && value.trim() === props.initial && <DoneLine>{t("common.saved")}</DoneLine>}
    </form>
  );
}
