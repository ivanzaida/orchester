import type { ComponentProps, ReactNode } from "react";

import { Icon, type IconName } from "./icon";
import styles from "./card.module.css";

/** The 400 px panel every page puts its content in (`G6OhQc`): 32 padding, 24 between sections. */
export function Card({ children }: { children: ReactNode }) {
  return <section className={styles.card}>{children}</section>;
}

/** The status pages' 40 px tile (`bQlNZ`): neutral, success or warning. */
export type TileTone = "neutral" | "success" | "warning";

/** Title and description, with the status tile above them on status pages (`WuzmL`, `UGQ9e`). */
export function Head(props: { title: string; description?: ReactNode; tile?: { icon: IconName; tone: TileTone } }) {
  return (
    <div className={styles.head}>
      {props.tile && (
        <span className={`${styles.tile} ${styles[props.tile.tone]}`}>
          <Icon name={props.tile.icon} size="tile" />
        </span>
      )}
      <h1 className={styles.title}>{props.title}</h1>
      {props.description !== undefined && <p className={styles.description}>{props.description}</p>}
    </div>
  );
}

/** The head and the address box under it (`U6c9Gf`). */
export function Top({ children }: { children: ReactNode }) {
  return <div className={styles.top}>{children}</div>;
}

/** The address a page is about, in a raised box (`f1C6s`). Raw data, not translated. */
export function Address({ email }: { email: string }) {
  return (
    <p className={styles.address}>
      <Icon name="at-sign" size="button" />
      <span className={styles.email}>{email}</span>
    </p>
  );
}

/** Running text and small print under it (`aSCJ8`). */
export function Body({ children }: { children: ReactNode }) {
  return <div className={styles.body}>{children}</div>;
}

export function Text({ children }: { children: ReactNode }) {
  return <p className={styles.text}>{children}</p>;
}

export function Small({ children }: { children: ReactNode }) {
  return <p className={styles.small}>{children}</p>;
}

/** A note with an icon at its first line, like the app's `Inspector note` (`vCguQ`, `VPiue`). */
export function Note({ icon, children }: { icon: IconName; children: ReactNode }) {
  return (
    <p className={styles.note}>
      <span className={styles.noteIcon}>
        <Icon name={icon} size="reason" />
      </span>
      <span>{children}</span>
    </p>
  );
}

/** The fields of a form, 16 apart (`TEKzl`). */
export function Fields({ children }: { children: ReactNode }) {
  return <div className={styles.fields}>{children}</div>;
}

/** The buttons and what is said about them, 10 apart (`FeF02`). */
export function Actions({ children }: { children: ReactNode }) {
  return <div className={styles.actions}>{children}</div>;
}

/** Two buttons side by side, sharing the width (`dMtAU`). */
export function ButtonRow({ children }: { children: ReactNode }) {
  return <div className={styles.row}>{children}</div>;
}

/** A form inside the card whose sections stay the card's own, 24 apart. */
export function CardForm(props: ComponentProps<"form">) {
  return <form className={styles.form} noValidate {...props} />;
}
