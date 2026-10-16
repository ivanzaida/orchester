import type { ReactNode } from "react";

import styles from "./confirm.module.css";

/**
 * A question asked in place, inside the row's card, never in a modal (`GsAoB`): what will happen, then the buttons
 * (keep first, the action second). The row above it is marked active.
 */
export function InPlaceConfirm(props: { title: string; text: string; children: ReactNode }) {
  return (
    <div className={styles.confirm} role="group" aria-label={props.title}>
      <p className={styles.title}>{props.title}</p>
      <p className={styles.text}>{props.text}</p>
      <div className={styles.buttons}>{props.children}</div>
    </div>
  );
}
