"use client";

import { useActionState } from "react";

import { Submit } from "@orchester/ui/components/submit";

import { signOutAction, type FormState } from "@/app/admin/actions";
import { t } from "@/lib/i18n";

import { ErrorLine } from "./fields";
import { CsrfField } from "./hidden";

/** *Sign out* (`TiVH0`): the header's ghost button, also in the phone's menu. */
export function SignOut({ csrf }: { csrf: string }) {
  const [state, action] = useActionState<FormState, FormData>(signOutAction, {});
  return (
    <form action={action}>
      <CsrfField token={csrf} />
      <Submit kind="ghost" row icon="log-out" label={t("header.signOut")} busyLabel={t("header.signingOut")} />
      {state.error && <ErrorLine message={state.error} />}
    </form>
  );
}
