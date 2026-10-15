import type { ReactNode } from "react";

import { Icon, type IconName } from "./icon";
import styles from "./list.module.css";

/**
 * Sections of rows, as on the Cloud's Account page (`H47EAc`) and the server admin's lists: a section title with a
 * count, a description, and a panel of rows. Every word comes from the app.
 */
export function Section(props: {
  id: string;
  title: string;
  meta?: string | undefined;
  description?: string;
  children: ReactNode;
}) {
  return (
    <section id={props.id} className={styles.section} aria-labelledby={`${props.id}-title`}>
      <div className={styles.head}>
        <div className={styles.titleRow}>
          <h2 id={`${props.id}-title`} className={styles.title}>
            {props.title}
          </h2>
          {props.meta !== undefined && <span className={styles.meta}>{props.meta}</span>}
        </div>
        {props.description !== undefined && <p className={styles.description}>{props.description}</p>}
      </div>
      {props.children}
    </section>
  );
}

/** The `$surface_panel` card the rows sit in, with a line between rows and none under the last. */
export function Panel({ children }: { children: ReactNode }) {
  return <div className={styles.panel}>{children}</div>;
}

/** A row: its 32 px glyph tile, its text, and what can be done with it (`aVRt4`, `R3smd`, `tB196`). */
export function Row(props: {
  glyph: IconName;
  align: "start" | "center";
  side?: ReactNode;
  /** The actions fill the row's width on a phone. */
  fill?: boolean;
  /** 14 above and below instead of 16, like the server admin's invite and device rows (`IF2QM`, `U7KMnL`). */
  compact?: boolean;
  /** A row being worked on, e.g. above its in-place confirmation (`kfxK0`). */
  active?: boolean;
  children: ReactNode;
}) {
  const classes = [styles.row, styles[props.align], props.compact && styles.compact, props.active && styles.active];
  return (
    <div className={classes.filter(Boolean).join(" ")}>
      <span className={styles.glyph} aria-hidden="true">
        <Icon name={props.glyph} size="glyph" />
      </span>
      <div className={styles.main}>{props.children}</div>
      {props.side !== undefined && <div className={`${styles.side}${props.fill ? ` ${styles.fill}` : ""}`}>{props.side}</div>}
    </div>
  );
}

/** A row replaced by what just happened to it (`VGUWi`): the success check, a title and a line. */
export function DoneRow({ title, text }: { title: string; text: string }) {
  return (
    <div className={`${styles.row} ${styles.start} ${styles.success}`} role="status">
      <span className={styles.successIcon}>
        <Icon name="circle-check" size="field" />
      </span>
      <div className={styles.main}>
        <span className={`${styles.name} ${styles.medium}`}>{title}</span>
        <span className={`${styles.line} ${styles.secondary} ${styles.wide}`}>{text}</span>
      </div>
    </div>
  );
}

/** A row's first line: its name, then chips. `weight`: 600 for invites and servers, 500 for devices. */
export function TopLine(props: { name: string; weight: "semibold" | "medium"; children?: ReactNode }) {
  return (
    <span className={styles.topLine}>
      <span className={`${styles.name}${props.weight === "medium" ? ` ${styles.medium}` : ""}`}>{props.name}</span>
      {props.children}
    </span>
  );
}

/** A line of a row: 12 muted; `secondary` for what matters more, `wide` for the 12.5 sentence of an invite. */
export function RowLine(props: { tone?: "muted" | "secondary"; wide?: boolean; children: ReactNode }) {
  const classes = [styles.line, props.tone === "secondary" && styles.secondary, props.wide && styles.wide];
  return <span className={classes.filter(Boolean).join(" ")}>{props.children}</span>;
}

/** A server's or project's address, as raw data in mono (`M6XUY`). */
export function AddressLine({ children }: { children: string }) {
  return <span className={styles.address}>{children}</span>;
}

export function Chip({ children }: { children: string }) {
  return <span className={styles.chip}>{children}</span>;
}

/** A state's tone: success (*Confirmed*, *Accepted*), warning (*Pending*), danger, info (*Admin*), neutral. */
export type Tone = "success" | "warning" | "danger" | "info" | "neutral";

/** A badge with its dot, like *Confirmed* (`eWGgO`, `Fm7lc`), in the tone of what it says. */
export function Badge({ children, tone = "success" }: { children: string; tone?: Tone }) {
  return (
    <span className={`${styles.badge} ${styles[`badge-${tone}`]}`}>
      <span className={styles.dot} aria-hidden="true" />
      {children}
    </span>
  );
}

/** A panel's empty state (`J9GyKs`): an icon, a title and a sentence, centred. */
export function EmptyState(props: { icon: IconName; title: string; text: string }) {
  return (
    <div className={styles.empty}>
      <Icon name={props.icon} size="empty" />
      <span className={styles.emptyTitle}>{props.title}</span>
      <span className={styles.emptyText}>{props.text}</span>
    </div>
  );
}

/** A setting: its label at the left (above on a phone) and its content (`szIXb`). */
export function SettingRow({ label, children }: { label: string; children: ReactNode }) {
  return (
    <div className={styles.setting}>
      <span className={styles.settingLabel}>{label}</span>
      <div className={styles.settingContent}>{children}</div>
    </div>
  );
}
