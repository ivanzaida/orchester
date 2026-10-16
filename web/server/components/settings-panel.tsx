import type { ReactNode } from "react";

import { Icon } from "@orchester/ui/components/icon";

import styles from "./settings-panel.module.css";

/** A panel of settings (`KjlRv`, `b2LXnC`): a label at the left (170 px), its content at the right. */
export function SettingsPanel({ children }: { children: ReactNode }) {
  return <div className={styles.panel}>{children}</div>;
}

export function SettingRow({ label, children }: { label: string; children: ReactNode }) {
  return (
    <div className={styles.row}>
      <span className={styles.label}>{label}</span>
      <div className={styles.content}>{children}</div>
    </div>
  );
}

/** What a setting means, under it (`u3X1cZ`). */
export function Hint({ children }: { children: ReactNode }) {
  return <p className={styles.hint}>{children}</p>;
}

/** A plain value in a setting (`Ivan Zaida, Alice Moreau`); `mono` for codes and keys. */
export function Value({ children, mono }: { children: ReactNode; mono?: boolean }) {
  return <span className={`${styles.value}${mono ? ` ${styles.mono}` : ""}`}>{children}</span>;
}

/** A value that cannot change, with its lock (the project's address). */
export function Locked({ children }: { children: ReactNode }) {
  return (
    <span className={styles.locked}>
      <Icon name="lock" size="note" />
      <span>{children}</span>
    </span>
  );
}
