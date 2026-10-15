import type { ReactNode } from "react";

import { Icon, type IconName } from "./icon";
import type { Tone } from "./list";
import styles from "./callout.module.css";

/**
 * A page-level message in a tinted box (`u7p3uL`, `fAZVq`, the states boards): an icon in the tone, a title, what it
 * means, and what can be done. Warnings and refusals use it; errors of one field stay under that field.
 */
export function Callout(props: {
  tone: Exclude<Tone, "info">;
  icon: IconName;
  title: string;
  body?: ReactNode;
  /** Actions beside the text on a wide page, under it on a phone. */
  action?: ReactNode;
  /** `alert` for something that went wrong just now; `status` otherwise. */
  role?: "alert" | "status";
}) {
  return (
    <div className={`${styles.callout} ${styles[props.tone]}`} role={props.role ?? "status"}>
      <span className={styles.icon}>
        <Icon name={props.icon} size="field" />
      </span>
      <div className={styles.text}>
        <p className={styles.title}>{props.title}</p>
        {props.body !== undefined && <p className={styles.body}>{props.body}</p>}
      </div>
      {props.action !== undefined && <div className={styles.action}>{props.action}</div>}
    </div>
  );
}
