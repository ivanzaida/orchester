"use client";

import { useActionState, useState } from "react";

import { ActionButton } from "@orchester/ui/components/button";
import { InPlaceConfirm } from "@orchester/ui/components/confirm";
import { Badge, DoneRow, Row, RowLine, TopLine } from "@orchester/ui/components/list";
import { Submit } from "@orchester/ui/components/submit";

import { ErrorLine } from "@/components/fields";
import { CsrfField, Hidden } from "@/components/hidden";
import { Phrase } from "@/components/when";
import { DAY_MS } from "@/lib/dates";
import { t } from "@/lib/i18n";
import type { Approval, Device, JoinedVia } from "@/lib/model";

import { approvalAction, revokeDeviceAction, type ApprovalState, type RevokeDeviceState } from "./actions";
import styles from "./devices.module.css";

const VIA: Readonly<Record<JoinedVia, "devices.viaCloud" | "devices.viaCode" | "devices.viaBefore">> = {
  cloud: "devices.viaCloud",
  code: "devices.viaCode",
  before_3: "devices.viaBefore",
};

/** A computer left unused this close to its 90 days says how long it has left (`ePtoG`). */
const WARN_UNUSED_DAYS = 76;
const NOW_MS = 120_000;

function LastUsed({ device, now }: { device: Device; now: number }) {
  const unused = Math.floor((now - device.lastUsedAt) / DAY_MS);
  if (now - device.lastUsedAt < NOW_MS) {
    return <>{t("devices.lastUsedNow")}</>;
  }
  if (unused >= WARN_UNUSED_DAYS) {
    return <>{t("devices.unused", { days: unused, left: Math.max(0, Math.ceil((device.expiresAt - now) / DAY_MS)) })}</>;
  }
  return <Phrase k="devices.lastUsed" dates={{ when: { at: device.lastUsedAt, format: "near" } }} now={now} />;
}

function Hiddens({ csrf, name, value, op }: { csrf: string; name: string; value: string; op?: string }) {
  return (
    <>
      <CsrfField token={csrf} />
      <Hidden name={name} value={value} />
      {op !== undefined && <Hidden name="op" value={op} />}
    </>
  );
}

/** A joined computer (`U7KMnL`), revoked after a question in place (`WV4AT`). */
export function DeviceRow({ device, me, server, csrf, now }: { device: Device; me: string; server: string; csrf: string; now: number }) {
  const [state, action] = useActionState<RevokeDeviceState, FormData>(revokeDeviceAction, {});
  const [asking, setAsking] = useState(false);
  const name = t("devices.deviceName", { device: device.device });
  if (state.revoked === true) {
    return <DoneRow title={t("devices.revokedTitle", { device: device.device })} text={t("devices.revokedText")} />;
  }
  const who = { name: device.user.displayName, username: device.user.username };
  const side = (
    <span className={styles.side}>
      <span className={styles.joined}>
        <RowLine tone="secondary" wide>{t(VIA[device.via])}</RowLine>
        <RowLine>
          <Phrase k="devices.joinedOn" dates={{ date: { at: device.createdAt, format: "date" } }} />
        </RowLine>
      </span>
      <span className={styles.used}>
        <LastUsed device={device} now={now} />
      </span>
      <span className={styles.actions}>
        <ActionButton kind="secondary" row expanded={asking} onClick={() => { setAsking(true); }}>
          {t("devices.revoke")}
        </ActionButton>
      </span>
    </span>
  );
  return (
    <>
      <Row glyph="laptop" align="center" compact active={asking} side={side}>
        <TopLine name={name} weight="semibold" />
        <RowLine>{device.user.id === me ? t("devices.whoYou", who) : t("devices.who", who)}</RowLine>
      </Row>
      {asking && (
        <InPlaceConfirm title={t("devices.revokeTitle", { device: device.device })} text={t("devices.revokeText", { server })}>
          <ActionButton kind="secondary" row autoFocus onClick={() => { setAsking(false); }}>
            {t("common.keep")}
          </ActionButton>
          <form action={action}>
            <Hiddens csrf={csrf} name="device" value={device.id} />
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

interface ApprovalProps {
  readonly approval: Approval;
  /** The person's username, from the admin's list of people; `null` when it is not there. */
  readonly username: string | null;
  readonly csrf: string;
  readonly now: number;
}

/** Who and from where (`JzUh8`): *Oleh Petrenko · oleh · joined through Orchester Cloud from 10.0.4.31, today 14:02*. */
function WaitingLine({ approval, username, now }: Omit<ApprovalProps, "csrf">) {
  const name = approval.user.displayName;
  const who = username === null ? name : t("devices.who", { name, username });
  const words = { name: who, address: approval.address ?? "" };
  const when = { when: { at: approval.requestedAt, format: "near" as const } };
  return (
    <RowLine>
      <Phrase k={approval.address === null ? "devices.waitingLineNoAddress" : "devices.waitingLine"} params={words} dates={when} now={now} />
    </RowLine>
  );
}

/** An answered computer (`WV4AT`): the row keeps its line with the badge of the answer, and what it means is under it. */
function Decided({ props, decided }: { props: ApprovalProps; decided: "approved" | "refused" }) {
  const { approval } = props;
  const approved = decided === "approved";
  return (
    <div className={styles.decided} role="status">
      <Row glyph="monitor-smartphone" align="center" compact>
        <TopLine name={t("devices.deviceName", { device: approval.device })} weight="semibold">
          <Badge tone={approved ? "success" : "danger"}>{approved ? t("devices.approvedBadge") : t("devices.refusedBadge")}</Badge>
        </TopLine>
        <WaitingLine approval={approval} username={props.username} now={props.now} />
      </Row>
      <p className={styles.decidedText}>
        {approved ? t("devices.approvedText", { name: approval.user.displayName }) : t("devices.refusedText")}
      </p>
    </div>
  );
}

/**
 * A computer waiting for join approval (`JzUh8`): *Approve* at once, *Refuse* after a question in place, while which
 * the row shows only its badge (`WV4AT`).
 */
export function ApprovalRow(props: ApprovalProps) {
  const { approval, csrf } = props;
  const [state, action] = useActionState<ApprovalState, FormData>(approvalAction, {});
  const [asking, setAsking] = useState(false);
  if (state.decided !== undefined) {
    return <Decided props={props} decided={state.decided} />;
  }
  const side = asking ? undefined : (
    <span className={styles.side}>
      <ActionButton kind="ghost" row expanded={asking} onClick={() => { setAsking(true); }}>
        {t("devices.refuse")}
      </ActionButton>
      <form action={action}>
        <Hiddens csrf={csrf} name="approval" value={approval.id} op="approve" />
        <Submit kind="primary" row icon="check" label={t("devices.approve")} busyLabel={t("devices.approving")} />
      </form>
    </span>
  );
  return (
    <>
      <Row glyph="monitor-smartphone" align="center" compact active={asking} side={side}>
        <TopLine name={t("devices.deviceName", { device: approval.device })} weight="semibold">
          <Badge tone="warning">{t("devices.waitingBadge")}</Badge>
        </TopLine>
        <WaitingLine approval={approval} username={props.username} now={props.now} />
      </Row>
      {asking && (
        <InPlaceConfirm title={t("devices.refuseTitle", { device: approval.device })} text={t("devices.refuseText", { name: approval.user.displayName })}>
          <ActionButton kind="secondary" row autoFocus onClick={() => { setAsking(false); }}>
            {t("devices.keepWaiting")}
          </ActionButton>
          <form action={action}>
            <Hiddens csrf={csrf} name="approval" value={approval.id} op="refuse" />
            <Submit kind="danger" row icon="x" label={t("devices.refuse")} busyLabel={t("devices.refusing")} />
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
