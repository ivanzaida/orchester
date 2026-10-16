import type { ReactNode } from "react";

import styles from "./frame.module.css";

/**
 * The three bars of `OLHng` *Brand / Orchester* in its raised tile, then the wordmark and the product tag (`V4PyD`);
 * the app gives both words.
 */
export function Brand({ name, product }: { name: string; product: string }) {
  return (
    <div className={styles.brand}>
      <Mark />
      <span className={styles.wordmark}>{name}</span>
      <span className={styles.tag}>{product}</span>
    </div>
  );
}

/** The brand's mark alone, its logo in the raised tile (`yTI2t`), as a phone's header shows it (`GZ2Kc`). */
export function Mark() {
  return (
    <span className={styles.mark} aria-hidden="true">
      <span className={styles.logo}>
        <span className={styles.bar1} />
        <span className={styles.bar2} />
        <span className={styles.bar3} />
      </span>
    </span>
  );
}

/** The 56 high footer (`szTC4`): what the site promises at the left, its links or notes at the right. */
export function Footer({ promise, aside }: { promise: string; aside: ReactNode }) {
  return (
    <footer className={styles.footer}>
      <p className={styles.promise}>{promise}</p>
      <span className={styles.spacer} />
      <span className={styles.links}>{aside}</span>
    </footer>
  );
}

/** A form page: the brand, one card and an optional line under it in a centred column, then the footer (`AmM6K`). */
export function CenteredPage(props: { brand: ReactNode; footer: ReactNode; children: ReactNode; below?: ReactNode }) {
  return (
    <>
      <main className={styles.main}>
        {props.brand}
        {props.children}
        {props.below}
      </main>
      {props.footer}
    </>
  );
}

/** *Already have an account? Sign in* under the card (`VBBaE`). */
export function SwitchLine({ question, link, href }: { question: string; link: string; href: string }) {
  return (
    <p className={styles.switch}>
      <span>{question}</span>
      <a className={styles.link} href={href}>
        {link}
      </a>
    </p>
  );
}
