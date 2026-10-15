import type { ReactNode } from "react";

import styles from "./table.module.css";

/**
 * A table of people in a panel (`mnZHf` head, `ZXrAd` member row, `CSg3J`/`LmtoQ` people): the app gives each its
 * columns as a class, so a head and its rows line up. On a phone the head hides and the cells wrap under the person.
 */
export function TableHead({ columns, children }: { columns: string | undefined; children: ReactNode }) {
  return (
    <div className={`${styles.head} ${columns ?? ""}`} role="row">
      {children}
    </div>
  );
}

export function HeadCell({ children, end, area }: { children?: ReactNode; end?: boolean; area?: string | undefined }) {
  return (
    <span className={[styles.headCell, end && styles.end, area].filter(Boolean).join(" ")} role="columnheader">
      {children}
    </span>
  );
}

export function TableRow({ columns, active, children }: { columns: string | undefined; active?: boolean; children: ReactNode }) {
  return (
    <div className={`${styles.row} ${columns ?? ""}${active ? ` ${styles.active}` : ""}`} role="row">
      {children}
    </div>
  );
}

/** `area`: the app's class naming the cell's grid area, so a phone can lay the row out in two lines. */
export function Cell({ children, end, area }: { children?: ReactNode; end?: boolean; area?: string | undefined }) {
  const classes = [styles.cell, end && styles.end, area].filter(Boolean).join(" ");
  return (
    <div className={classes} role="cell">
      {children}
    </div>
  );
}
