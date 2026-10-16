import type { ReactNode } from "react";

import styles from "./facts.module.css";

export interface Fact {
  readonly label: string;
  readonly value: string;
  /** Codes, fingerprints and addresses are mono. */
  readonly mono?: boolean;
}

/**
 * A mono value that may wrap only after its separators (`/`, `:`, `.`, `-`), never inside a token: an address on a
 * phone breaks as `https://` / `orchester.kontur.dev`, not in the middle of its port.
 */
function breakable(value: string): ReactNode[] {
  return value.split(/(?<=[/:.-])/).flatMap((part, index) => (index === 0 ? [part] : [<wbr key={index} />, part]));
}

/**
 * Label and value pairs (`YEvFE` in a code shown once, `tKXRn` in the certificate check): `wide` labels are 130 px,
 * `narrow` 90; `boxed` puts them in the input-coloured box.
 */
export function Facts({ items, labels, boxed }: { items: readonly Fact[]; labels: "wide" | "narrow"; boxed?: boolean }) {
  return (
    <dl className={`${styles.facts} ${styles[labels]}${boxed ? ` ${styles.boxed}` : ""}`}>
      {items.map((fact) => (
        <div key={fact.label} className={styles.fact}>
          <dt className={styles.label}>{fact.label}</dt>
          <dd className={`${styles.value}${fact.mono ? ` ${styles.mono}` : ""}`}>{fact.mono ? breakable(fact.value) : fact.value}</dd>
        </div>
      ))}
    </dl>
  );
}
