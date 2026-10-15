import type { ReactNode } from "react";

import { Icon } from "./icon";
import styles from "./status.module.css";

/** The app's `Field error` (`d5xipm`): red icon and words, under a field or for a whole form. */
export function ErrorLine({ id, children }: { id?: string | undefined; children: ReactNode }) {
  return (
    <p id={id} className={`${styles.line} ${styles.error}`} role="alert">
      <Icon name="circle-alert" size="reason" />
      <span>{children}</span>
    </p>
  );
}

/** Why a button cannot be pressed now, like the app's `Unavailable reason` (`QR1ER`). */
export function Reason({ id, children }: { id?: string; children: ReactNode }) {
  return (
    <p id={id} className={`${styles.line} ${styles.reason}`}>
      <span className={styles.lineIcon}>
        <Icon name="lock" size="reason" />
      </span>
      <span>{children}</span>
    </p>
  );
}

/** Something just done, with the success check (`X90j6I`). */
export function DoneLine({ children }: { children: ReactNode }) {
  return (
    <p className={`${styles.line} ${styles.done}`} role="status">
      <Icon name="circle-check" size="button" />
      <span>{children}</span>
    </p>
  );
}

/** The app's warning `Notice` (`R3Pyx`, `optfF`): an icon, a title, what to do, and one action. */
export function Notice(props: { title: string; body: string; action: ReactNode }) {
  return (
    <div className={styles.notice} role="status">
      <Icon name="mail-warning" size="field" />
      <div className={styles.noticeText}>
        <div className={styles.noticeCopy}>
          <p className={styles.noticeTitle}>{props.title}</p>
          <p className={styles.noticeBody}>{props.body}</p>
        </div>
        {props.action}
      </div>
    </div>
  );
}
