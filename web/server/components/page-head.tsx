import type { ReactNode } from "react";

import { Icon } from "@orchester/ui/components/icon";

import styles from "./page-head.module.css";

/** A page's title, what it is for, and its main action at the right (`T7GSXZ`). */
export function PageHead(props: { title: string; description?: ReactNode; actions?: ReactNode; under?: ReactNode }) {
  return (
    <div className={styles.head}>
      <div className={styles.text}>
        <h1 className={styles.title}>{props.title}</h1>
        {props.under}
        {props.description !== undefined && <p className={styles.description}>{props.description}</p>}
      </div>
      {props.actions !== undefined && <div className={styles.actions}>{props.actions}</div>}
    </div>
  );
}

/** A quiet note under a list (`SOlAA`): the info icon and a sentence. */
export function Note({ children }: { children: ReactNode }) {
  return (
    <p className={styles.note}>
      <span className={styles.noteIcon}>
        <Icon name="info" size="note" />
      </span>
      <span>{children}</span>
    </p>
  );
}
