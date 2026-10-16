"use client";

import { useState } from "react";

import { Callout } from "@orchester/ui/components/callout";
import { Fields } from "@orchester/ui/components/card";
import { useRemaining } from "@orchester/ui/hooks/countdown";
import { Submit } from "@orchester/ui/components/submit";
import { useCheckedForm } from "@orchester/ui/hooks/use-form";

import { ErrorLine, TextField } from "@/components/fields";
import { CsrfField } from "@/components/hidden";
import { Say } from "@/components/say";
import { message, t } from "@/lib/i18n";

import { signInCodeAction, type CodeState } from "./actions";
import styles from "./code.module.css";

const START: CodeState = { attempt: 0, code: "", outcome: { kind: "none" } };

/**
 * The code form (`ulIoG`; wrong, used or expired, too many in `j8WujU`). A wrong code is marked on the field and kept
 * so it can be corrected; too many wrong codes stop the button until the time the server gave.
 */
export function CodeForm({ csrf }: { csrf: string }) {
  const { state, pending, form } = useCheckedForm(signInCodeAction, START);
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
        <TextField name="code" type="text" size="row" label={t("code.field")} value={code} onChange={setCode} hint={t("code.hint")}
          error={outcome.kind === "wrong" ? outcome.error : undefined} autoComplete="one-time-code" mono />
        {limited && (
          <Callout tone="danger" icon="circle-alert" title={t("code.tooManyTitle")} role="alert"
            body={<Say message={message("code.tooManyText", {}, outcome.until)} />} />
        )}
        {outcome.kind === "failed" && <ErrorLine message={outcome.error} />}
      </Fields>
      <Submit kind="primary" icon="log-in" label={t("code.submit")} busyLabel={t("code.submitting")} pending={pending}
        unavailable={limited} />
    </form>
  );
}
