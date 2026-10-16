"use client";

import { useActionState, useEffect, useRef } from "react";

import type { ServerView } from "@/lib/authorize";
import { t } from "@/lib/i18n";
import { Avatar } from "@orchester/ui/components/avatar";
import { buttonClass, ButtonContent, ButtonLink } from "@orchester/ui/components/button";
import { Card, CardForm, Head } from "@orchester/ui/components/card";
import { Icon } from "@orchester/ui/components/icon";
import { Submit } from "@orchester/ui/components/submit";
import { ErrorLine } from "@/components/fields";
import { CsrfField, Hidden } from "@/components/hidden";

import { authorizeAction, type AuthorizeState } from "./actions";
import styles from "./authorize.module.css";

export interface AuthorizeProps {
  readonly csrf: string;
  readonly purpose: "admin" | "link";
  readonly person: { readonly name: string; readonly email: string; readonly initials: string };
  readonly server: ServerView;
  /** The server's host, as the page names it (`orchester.kontur.dev`). */
  readonly host: string;
  readonly notYou: string;
  readonly cancel: string;
}

function Person({ person, notYou }: Pick<AuthorizeProps, "person" | "notYou">) {
  return (
    <div className={styles.person}>
      <Avatar initials={person.initials} size="header" />
      <div className={styles.who}>
        <span className={styles.name}>{person.name}</span>
        <span className={styles.muted}>{person.email}</span>
      </div>
      <a className={styles.notYou} href={notYou}>
        {t("authorize.notYou")}
      </a>
    </div>
  );
}

function Server({ purpose, server, host }: Pick<AuthorizeProps, "purpose" | "server" | "host">) {
  return (
    <div className={styles.server}>
      <div className={styles.line}>
        <Icon name="server" size="button" />
        <span className={styles.name}>{server.name}</span>
        <span className={styles.address}>{host}</span>
      </div>
      {purpose === "admin" && <span className={styles.muted}>{t("authorize.backTo", { place: `${host}/admin` })}</span>}
      {purpose === "link" && server.checkCode !== null && (
        <>
          <div className={styles.check}>
            <span className={styles.checkLabel}>{t("authorize.checkCode")}</span>
            <span className={styles.code}>{server.checkCode}</span>
          </div>
          <span className={styles.muted}>{t("authorize.checkCodeSame")}</span>
        </>
      )}
    </div>
  );
}

/**
 * Carries the answer to the server admin: a form post to its callback, sent as soon as the page has it; without
 * script the person presses *Continue*.
 */
function Carry({ action, fields }: { action: string; fields: readonly (readonly [string, string])[] }) {
  const form = useRef<HTMLFormElement>(null);
  useEffect(() => {
    form.current?.submit();
  }, []);
  return (
    <form ref={form} method="post" action={action}>
      {fields.map(([name, value]) => (
        <Hidden key={name} name={name} value={value} />
      ))}
      <button type="submit" className={buttonClass("primary")}>
        <ButtonContent icon="log-in">{t("authorize.continue")}</ButtonContent>
      </button>
    </form>
  );
}

const READY: AuthorizeState = { kind: "ready" };

/** *Sign in to Kontur team's admin as …?* (`sssAw`) and *Link Kontur team to your account?* (`WNc6d`). */
export function AuthorizeCard(props: AuthorizeProps) {
  const [state, action] = useActionState(authorizeAction, READY);
  const admin = props.purpose === "admin";
  const title = admin
    ? t("authorize.adminTitle", { server: props.server.name, email: props.person.email })
    : t("authorize.linkTitle", { server: props.server.name });
  const description = admin ? t("authorize.adminDescription", { server: props.server.name }) : t("authorize.linkDescription");
  return (
    <div className={styles.wide}>
      <Card>
        <div className={styles.parts}>
          <Head title={title} description={description} />
          <Person person={props.person} notYou={props.notYou} />
          <Server purpose={props.purpose} server={props.server} host={props.host} />
          {state.kind === "go" ? (
            <Carry action={state.action} fields={state.fields} />
          ) : (
            <CardForm action={action}>
              <CsrfField token={props.csrf} />
              <div className={styles.buttons}>
                {state.kind === "failed" && <ErrorLine message={state.error} />}
                <Submit kind="primary" icon={admin ? undefined : "link"}
                  label={admin ? t("authorize.adminSubmit") : t("authorize.linkSubmit", { server: props.server.name })}
                  busyLabel={t("authorize.submitting")} />
                <ButtonLink kind="ghost" href={props.cancel}>
                  {t("authorize.cancel")}
                </ButtonLink>
              </div>
            </CardForm>
          )}
        </div>
      </Card>
    </div>
  );
}
