import { ButtonLink } from "@orchester/ui/components/button";
import { Icon } from "@orchester/ui/components/icon";

import { t } from "@/lib/i18n";
import type { Approval } from "@/lib/model";

import styles from "./waiting.module.css";

/** *A computer is waiting for approval* (`YMZ6N`): on the Projects page, for admins, with the way to Devices. */
export function WaitingCallout({ approvals }: { approvals: readonly Approval[] }) {
  const [first] = approvals;
  if (first === undefined) {
    return null;
  }
  const one = approvals.length === 1;
  return (
    <div className={styles.waiting} role="status">
      <span className={styles.glyph} aria-hidden="true">
        <Icon name="monitor-smartphone" size="glyph" />
      </span>
      <div className={styles.text}>
        <p className={styles.title}>
          {one ? t("projects.waitingTitle") : t("projects.waitingTitleMany", { count: approvals.length })}
        </p>
        <p className={styles.body}>
          {one ? t("projects.waitingText", { device: first.device, name: first.user.displayName }) : t("projects.waitingTextMany")}
        </p>
      </div>
      <ButtonLink kind="secondary" row href="/admin/devices">
        {t("projects.review")}
      </ButtonLink>
    </div>
  );
}
