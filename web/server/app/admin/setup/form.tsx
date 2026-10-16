"use client";

import { useState } from "react";

import { Callout } from "@orchester/ui/components/callout";
import { Fields } from "@orchester/ui/components/card";
import { Submit } from "@orchester/ui/components/submit";
import { useRemaining } from "@orchester/ui/hooks/countdown";
import { useCheckedForm } from "@orchester/ui/hooks/use-form";

import { ErrorLine, TextField } from "@/components/fields";
import { CsrfField } from "@/components/hidden";
import { Say } from "@/components/say";
import { message, t, type Message } from "@/lib/i18n";

import { setupCodeAction, type SetupState } from "./actions";
import styles from "./setup.module.css";

const START: SetupState = { attempt: 0, code: "", outcome: { kind: "none" } };

/** Setup step 1 (`t9q0y`; wrong or expired `j8WujU`): the setup code from the server's console. */
export function SetupCodeForm({ csrf, notice }: { csrf: string; notice: Message | null }) {
  const { state, pending, form } = useCheckedForm(setupCodeAction, START);
  const [code, setCode] = useState(state.code);
  const [seen, setSeen] = useState(state.attempt);
  if (seen !== state.attempt) {
    setSeen(state.attempt);
    setCode(state.code);
  }
  const { outcome } = state;
  const wait = useRemaining(outcome.kind === "limited" ? outcome.ms : 0);
  const limited = outcome.kind === "limited" && wait > 0;
  return (
    <form className={styles.form} {...form} noValidate>
      <CsrfField token={csrf} />
      <Fields>
        {notice !== null && state.attempt === 0 && <ErrorLine message={notice} />}
        <TextField name="code" type="text" size="row" label={t("setup.field")} value={code} onChange={setCode} hint={t("setup.hint")}
          error={outcome.kind === "wrong" ? outcome.error : undefined} autoComplete="one-time-code" mono />
        {limited && (
          <Callout tone="danger" icon="circle-alert" title={t("setup.tooManyTitle")} role="alert"
            body={<Say message={message("setup.tooManyText", {}, outcome.until)} />} />
        )}
        {outcome.kind === "failed" && <ErrorLine message={outcome.error} />}
      </Fields>
      <Submit kind="primary" label={t("setup.continue")} busyLabel={t("setup.continuing")} pending={pending} unavailable={limited} />
    </form>
  );
}
