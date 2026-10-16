"use client";

import { useActionState, useState } from "react";

import { ActionButton } from "@orchester/ui/components/button";
import { InPlaceConfirm } from "@orchester/ui/components/confirm";
import { Badge, DoneRow, Row, RowLine, TopLine } from "@orchester/ui/components/list";
import { Submit } from "@orchester/ui/components/submit";

import { ErrorLine } from "@/components/fields";
import { CsrfField, Hidden } from "@/components/hidden";
import { Phrase } from "@/components/when";
import { t } from "@/lib/i18n";
import type { AdminBrowser } from "@/lib/model";

import { revokeBrowserAction, type RevokeDeviceState } from "./actions";
import styles from "./devices.module.css";

/**
 * A browser signed in to the admin, signed out after a question in place. Not drawn: the Joined row (`U7KMnL`) with
 * the person as its name and the sign-in's 12 hours as its line (security review 1, minor 5).
 */
export function BrowserRow({ browser, csrf, now }: { browser: AdminBrowser; csrf: string; now: number }) {
  const [state, action] = useActionState<RevokeDeviceState, FormData>(revokeBrowserAction, {});
  const [asking, setAsking] = useState(false);
  const name = browser.user.displayName;
  if (state.revoked === true) {
    return <DoneRow title={t("devices.browserRevokedTitle", { name })} text={t("devices.browserRevokedText")} />;
  }
  const dates = { when: { at: browser.createdAt, format: "near" as const }, end: { at: browser.expiresAt, format: "time" as const } };
  const side = (
    <span className={styles.side}>
      <span className={styles.used}>
        <Phrase k="devices.browserSince" dates={dates} now={now} />
      </span>
      <span className={styles.actions}>
        {browser.current ? (
          <Badge tone="neutral">{t("devices.thisBrowser")}</Badge>
        ) : (
          <ActionButton kind="secondary" row onClick={() => { setAsking(true); }}>
            {t("devices.revoke")}
          </ActionButton>
        )}
      </span>
    </span>
  );
  return (
    <>
      <Row glyph="app-window" align="center" compact active={asking} side={side}>
        <TopLine name={name} weight="semibold" />
        <RowLine>{t("devices.browserWho", { username: browser.user.username })}</RowLine>
      </Row>
      {asking && (
        <InPlaceConfirm title={t("devices.browserRevokeTitle", { name })} text={t("devices.browserRevokeText")}>
          <ActionButton kind="secondary" row onClick={() => { setAsking(false); }}>
            {t("common.keep")}
          </ActionButton>
          <form action={action}>
            <CsrfField token={csrf} />
            <Hidden name="browser" value={browser.id} />
            <Submit kind="danger" row icon="x" label={t("devices.revoke")} busyLabel={t("devices.revoking")} />
          </form>
        </InPlaceConfirm>
      )}
      {state.error !== undefined && (
        <div className={styles.under}>
          <ErrorLine message={state.error} />
        </div>
      )}
    </>
  );
}
