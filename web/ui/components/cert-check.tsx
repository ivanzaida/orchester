import { Facts, type Fact } from "./facts";
import { Icon } from "./icon";
import styles from "./cert-check.module.css";

/**
 * The first-visit certificate hint (`nNRnL`): the page cannot prove itself, so the person compares the certificate's
 * SHA-256 in the browser with what the server's console prints. Every word comes from the app.
 */
export function CertificateCheck(props: { title: string; body: string; facts: readonly Fact[]; footnote: string }) {
  return (
    <aside className={styles.check} aria-label={props.title}>
      <p className={styles.head}>
        <Icon name="shield-question" size="glyph" />
        <span className={styles.title}>{props.title}</span>
      </p>
      <p className={styles.body}>{props.body}</p>
      <Facts items={props.facts} labels="narrow" boxed />
      <p className={styles.footnote}>{props.footnote}</p>
    </aside>
  );
}
