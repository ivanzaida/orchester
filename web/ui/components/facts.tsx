import styles from "./facts.module.css";

export interface Fact {
  readonly label: string;
  readonly value: string;
  /** Codes, fingerprints and addresses are mono. */
  readonly mono?: boolean;
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
          <dd className={`${styles.value}${fact.mono ? ` ${styles.mono}` : ""}`}>{fact.value}</dd>
        </div>
      ))}
    </dl>
  );
}
