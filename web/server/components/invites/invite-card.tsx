import type { ReactNode } from "react";

import { Icon, type IconName } from "@orchester/ui/components/icon";

import styles from "./invites.module.css";

/** A way to invite (`vzkko` by email, `Vo87V` with a code, `PPiT4` to the server): glyph, title, what it does, form. */
export function InviteCard(props: { id: string; icon: IconName; title: string; text: string; children: ReactNode }) {
  return (
    <section className={styles.card} aria-labelledby={`${props.id}-title`}>
      <div className={styles.cardHead}>
        <span className={styles.glyph} aria-hidden="true">
          <Icon name={props.icon} size="glyph" />
        </span>
        <div className={styles.cardText}>
          <h2 id={`${props.id}-title`} className={styles.cardTitle}>
            {props.title}
          </h2>
          <p className={styles.cardBody}>{props.text}</p>
        </div>
      </div>
      <div className={styles.indent}>{props.children}</div>
    </section>
  );
}
