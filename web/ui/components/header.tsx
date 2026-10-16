import type { ReactNode } from "react";

import styles from "./header.module.css";

/**
 * A site's 64 px header (`NNrob`, the Cloud's `VSR5X`): the brand, what the site is about (a server's name and
 * address), the signed-in person and their action at the right. On a phone (`nxYvC`) it keeps the mark, the context
 * and the person's initials, and moves the rest into `menu`.
 */
export function SiteHeader(props: {
  brand: ReactNode;
  mark: ReactNode;
  context?: ReactNode;
  who?: ReactNode;
  avatar?: ReactNode;
  action?: ReactNode;
  menu?: ReactNode;
}) {
  return (
    <header className={styles.header}>
      <div className={styles.wide}>{props.brand}</div>
      <div className={styles.compact}>{props.mark}</div>
      {props.context !== undefined && (
        <>
          <span className={`${styles.divider} ${styles.wide}`} aria-hidden="true" />
          <div className={styles.context}>{props.context}</div>
        </>
      )}
      <span className={styles.spacer} />
      {props.who !== undefined && <div className={`${styles.who} ${styles.wide}`}>{props.who}</div>}
      {props.avatar !== undefined && <div className={styles.compact}>{props.avatar}</div>}
      {props.action !== undefined && (
        <>
          <span className={`${styles.divider} ${styles.wide}`} aria-hidden="true" />
          <div className={styles.wide}>{props.action}</div>
        </>
      )}
      {props.menu !== undefined && <div className={styles.compact}>{props.menu}</div>}
    </header>
  );
}

/** The two lines beside an avatar or after the brand: a name and, under it, an address or a role. */
export function HeaderLines(props: { title: string; line: string; mono?: boolean }) {
  return (
    <span className={styles.lines}>
      <span className={styles.title}>{props.title}</span>
      <span className={`${styles.line}${props.mono ? ` ${styles.mono}` : ""}`}>{props.line}</span>
    </span>
  );
}
