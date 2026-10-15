"use client";

import { useActionState } from "react";

import { message, t } from "@/lib/i18n";
import { formatWait } from "@/lib/resend";
import { ButtonLink } from "@/ui/button";
import { Actions, ButtonRow } from "@/ui/card";
import { useRemaining } from "@/ui/countdown";
import { CsrfField } from "@/ui/hidden";
import { Say } from "@/ui/say";
import { DoneLine, ErrorLine, Reason } from "@/ui/status";
import { Submit } from "@/ui/submit";

import { resendAction, type ResendState } from "./actions";
import styles from "./resend.module.css";

export interface ResendProps {
  csrf: string;
  /** When the last email went out, if it was a resend (*Sent again at …*, `o62WH`). */
  resentAt?: number | undefined;
  /** What is left of the 60 s between emails (`y4FZY`). */
  cooldownMs: number;
  /** Set while the cloud refuses more emails (`PLwGT`): until when, and how long that is from now. */
  limit?: { until: number; ms: number } | undefined;
}

function Why({ cooldown, limit }: { cooldown: number; limit: ResendProps["limit"] }) {
  if (limit !== undefined) {
    return (
      <Reason id="resend-reason">
        <Say message={message("sent.limit", {}, limit.until)} />
      </Reason>
    );
  }
  return cooldown > 0 ? <Reason id="resend-reason">{t("sent.cooldown", { wait: formatWait(cooldown) })}</Reason> : null;
}

/** *Resend email* and *Use another email* with the reason the first cannot be pressed yet. */
export function ResendPanel(props: ResendProps) {
  const [state, action] = useActionState<ResendState, FormData>(resendAction, {});
  const cooldown = useRemaining(props.cooldownMs);
  const limitLeft = useRemaining(props.limit?.ms ?? 0);
  const limit = props.limit !== undefined && limitLeft > 0 ? props.limit : undefined;
  const waiting = limit !== undefined || cooldown > 0;
  return (
    <Actions>
      {props.resentAt !== undefined && limit === undefined && (
        <DoneLine>
          <Say message={message("sent.sentAgain", {}, props.resentAt)} />
        </DoneLine>
      )}
      {state.error && <ErrorLine message={state.error} />}
      <ButtonRow>
        <form className={styles.form} action={action}>
          <CsrfField token={props.csrf} />
          <Submit kind="secondary" icon="refresh-cw" label={t("sent.resend")} busyLabel={t("sent.resending")}
            unavailable={waiting} describedBy={waiting ? "resend-reason" : undefined} />
        </form>
        <ButtonLink kind="ghost" href="/signup">
          {t("sent.useAnother")}
        </ButtonLink>
      </ButtonRow>
      <Why cooldown={cooldown} limit={limit} />
    </Actions>
  );
}
