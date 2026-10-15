"use client";

import { useActionState } from "react";

import { Submit } from "@orchester/ui/components/submit";

import { ErrorLine } from "@/components/fields";
import { CsrfField } from "@/components/hidden";
import { t } from "@/lib/i18n";

import { signOutAction, type FormState } from "./actions";
import styles from "./account.module.css";

/** Sign out of this browser: the header's ghost button (`Mj5sB`) and the Account section's secondary one (`Nb8ti`). */
export function SignOutForm(props: { csrf: string; kind: "ghost" | "secondary"; label: string; icon?: boolean }) {
  const { csrf, kind, label } = props;
  const [state, action] = useActionState<FormState, FormData>(signOutAction, {});
  return (
    <form action={action}>
      <CsrfField token={csrf} />
      <Submit kind={kind} row {...(props.icon === false ? {} : { icon: "log-out" as const })} label={label}
        busyLabel={t("account.signingOut")} />
      {state.error && <ErrorLine message={state.error} />}
    </form>
  );
}

function initials(name: string): string {
  return name
    .split(/\s+/)
    .filter(Boolean)
    .slice(0, 2)
    .map((word) => word.charAt(0).toUpperCase())
    .join("");
}

/** The 64 px header (`VSR5X`): the brand, the signed-in account and *Sign out*; on a phone the account goes below. */
export function AccountHeader(props: { brand: React.ReactNode; name: string; email: string; csrf: string }) {
  return (
    <header className={styles.header}>
      <div className={styles.brand}>{props.brand}</div>
      <div className={styles.who}>
        <span className={styles.avatar} aria-hidden="true">
          {initials(props.name)}
        </span>
        <span className={styles.identity}>
          <span className={styles.name}>{props.name}</span>
          <span className={styles.email}>{props.email}</span>
        </span>
      </div>
      <span className={styles.divider} aria-hidden="true" />
      <div className={styles.out}>
        <SignOutForm csrf={props.csrf} kind="ghost" label={t("account.signOut")} />
      </div>
    </header>
  );
}
