import type { ReactNode } from "react";

import styles from "./table.module.css";

/**
 * A table of people in a panel (`mnZHf` head, `ZXrAd` member row, `CSg3J`/`LmtoQ` people): the app gives each its
 * columns as a class, so a head and its rows line up. On a phone the head hides and the cells wrap under the person.
 */
export function TableHead({ columns, children }: { columns: string; children: ReactNode }) {
  return (
    <div className={`${styles.head} ${columns}`} role="row">
      {children}
    </div>
  );
}

export function HeadCell({ children, end }: { children?: ReactNode; end?: boolean }) {
  return (
    <span className={`${styles.headCell}${end ? ` ${styles.end}` : ""}`} role="columnheader">
      {children}
    </span>
  );
}

export function TableRow({ columns, active, children }: { columns: string; active?: boolean; children: ReactNode }) {
  return (
    <div className={`${styles.row} ${columns}${active ? ` ${styles.active}` : ""}`} role="row">
      {children}
    </div>
  );
}

export function Cell({ children, end }: { children?: ReactNode; end?: boolean }) {
  return (
    <div className={`${styles.cell}${end ? ` ${styles.end}` : ""}`} role="cell">
      {children}
    </div>
  );
}
