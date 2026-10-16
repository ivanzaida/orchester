"use client";

import { useEffect, useState } from "react";

import { ButtonLink } from "@orchester/ui/components/button";
import { Actions, Card, CardForm, Fields, Head } from "@orchester/ui/components/card";
import { Submit } from "@orchester/ui/components/submit";
import { useCheckedForm } from "@orchester/ui/hooks/use-form";

import { ErrorLine, TextField } from "@/components/fields";
import { CsrfField } from "@/components/hidden";
import { t } from "@/lib/i18n";

import { codeAction, type CodeState } from "./actions";

const START: CodeState = { attempt: 0, outcome: { kind: "none" } };

/** A sign-in that ended before its code (five minutes, five codes, or a new password meanwhile): sign in again. */
export function CodeExpired() {
  return (
    <Card>
      <Head title={t("signin.code.expiredTitle")} description={t("signin.code.expiredDescription")}
        tile={{ icon: "clock", tone: "warning" }} />
      <ButtonLink href="/signin" kind="primary" icon="log-in">
        {t("signin.code.signInAgain")}
      </ButtonLink>
    </Card>
  );
}

/**
 * The sign-in's second step: the code from the authenticator app for `email`, or one of its recovery codes; `null`
 * when no sign-in waits in this browser. The email the page opened with is kept: a right code ends the challenge, and
 * the page drawn again then has none, while the form still goes on to `/authorize` with a whole page load.
 */
export function CodeForm({ csrf, email }: { csrf: string; email: string | null }) {
  const [shown] = useState(email);
  const { state, pending, form } = useCheckedForm(codeAction, START);
  const [code, setCode] = useState("");
  const [seen, setSeen] = useState(state.attempt);
  if (seen !== state.attempt) {
    setSeen(state.attempt);
    setCode("");
  }
  const outcome = state.outcome;
  useEffect(() => {
    if (outcome.kind === "next") {
      window.location.assign(outcome.href);
    }
  }, [outcome]);
  if (outcome.kind === "expired" || shown === null) {
    return <CodeExpired />;
  }
  return (
    <Card>
      <Head title={t("signin.code.title")} description={t("signin.code.description", { email: shown })}
        tile={{ icon: "shield", tone: "neutral" }} />
      <CardForm {...form}>
        <CsrfField token={csrf} />
        <Fields>
          <TextField name="code" type="text" label={t("signin.code.label")} value={code} onChange={setCode}
            hint={t("signin.code.hint")} autoComplete="one-time-code" code />
          {outcome.kind === "wrong" && <ErrorLine message={{ key: "signin.code.wrong" }} />}
          {outcome.kind === "failed" && <ErrorLine message={outcome.error} />}
        </Fields>
        <Actions>
          <Submit kind="primary" icon="log-in" label={t("signin.code.submit")} busyLabel={t("signin.code.submitting")}
            pending={pending} unavailable={code.trim() === ""} />
        </Actions>
      </CardForm>
    </Card>
  );
}
