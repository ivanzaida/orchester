import { ButtonLink } from "@orchester/ui/components/button";
import { Icon } from "@orchester/ui/components/icon";

import { PhoneOnly, WideOnly } from "@/components/only";
import { t } from "@/lib/i18n";
import type { Approval } from "@/lib/model";

import phone from "./phone.module.css";
import styles from "./waiting.module.css";

function title(approvals: readonly Approval[]): string {
  return approvals.length === 1 ? t("projects.waitingTitle") : t("projects.waitingTitleMany", { count: approvals.length });
}

/** On a phone (`A7hWOC`): the glyph inline, the short sentence, and *Review in Devices* as a link. */
function PhoneWaiting({ approvals, first }: { approvals: readonly Approval[]; first: Approval }) {
  return (
    <div className={phone.waiting} role="status">
      <span className={phone.waitingIcon} aria-hidden="true">
        <Icon name="monitor-smartphone" size="field" />
      </span>
      <div className={phone.waitingText}>
        <p className={phone.waitingTitle}>{title(approvals)}</p>
        <p className={phone.waitingBody}>
          {approvals.length === 1
            ? t("projects.waitingTextPhone", { device: first.device, name: first.user.displayName })
            : t("projects.waitingTextMany")}
        </p>
        <a className={phone.review} href="/admin/devices">
          {t("projects.review")}
        </a>
      </div>
    </div>
  );
}

/** *A computer is waiting for approval* (`YMZ6N`; `A7hWOC` on a phone): on Projects, for admins, with the way to Devices. */
export function WaitingCallout({ approvals }: { approvals: readonly Approval[] }) {
  const [first] = approvals;
  if (first === undefined) {
    return null;
  }
  const one = approvals.length === 1;
  return (
    <>
      <WideOnly>
        <WideWaiting approvals={approvals} first={first} one={one} />
      </WideOnly>
      <PhoneOnly>
        <PhoneWaiting approvals={approvals} first={first} />
      </PhoneOnly>
    </>
  );
}

function WideWaiting({ approvals, first, one }: { approvals: readonly Approval[]; first: Approval; one: boolean }) {
  return (
    <div className={styles.waiting} role="status">
      <span className={styles.glyph} aria-hidden="true">
        <Icon name="monitor-smartphone" size="glyph" />
      </span>
      <div className={styles.text}>
        <p className={styles.title}>{title(approvals)}</p>
        <p className={styles.body}>
          {one ? t("projects.waitingText", { device: first.device, name: first.user.displayName }) : t("projects.waitingTextMany")}
        </p>
      </div>
      <span className={styles.action}>
        <ButtonLink kind="secondary" row href="/admin/devices">
          {t("projects.review")}
        </ButtonLink>
      </span>
    </div>
  );
}
