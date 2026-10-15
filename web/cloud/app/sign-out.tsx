"use client";

import { useActionState } from "react";

import { t } from "@/lib/i18n";
import { Actions } from "@orchester/ui/components/card";
import { ErrorLine } from "@/components/fields";
import { CsrfField } from "@/components/hidden";
import { Submit } from "@orchester/ui/components/submit";

import { signOutAction, type SignOutState } from "./actions";

export function SignOut({ csrf }: { csrf: string }) {
  const [state, action] = useActionState<SignOutState, FormData>(signOutAction, {});
  return (
    <form action={action}>
      <CsrfField token={csrf} />
      <Actions>
        {state.error && <ErrorLine message={state.error} />}
        <Submit kind="secondary" icon="log-out" label={t("home.signOut")} busyLabel={t("home.signingOut")} />
      </Actions>
    </form>
  );
}
