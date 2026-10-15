import { CopyButton } from "./copy";
import { Facts, type Fact } from "./facts";
import { Icon } from "./icon";
import { Badge } from "./list";
import styles from "./code-once.module.css";

/**
 * A code shown once (`bdPlP`): what it is for, the code large with *Copy code*, the facts the person needs with it
 * (address, check code, expiry) and the warning that it is not shown again. It exists only in the page that made it.
 */
export function CodeShownOnce(props: {
  title: string;
  badge: string;
  code: string;
  copyLabel: string;
  copiedLabel: string;
  facts: readonly Fact[];
  warning: string;
}) {
  return (
    <section className={styles.card} aria-label={props.title}>
      <div className={styles.head}>
        <Icon name="key-round" size="glyph" />
        <h3 className={styles.title}>{props.title}</h3>
        <span className={styles.spacer} />
        <Badge tone="warning">{props.badge}</Badge>
      </div>
      <div className={styles.codeLine}>
        <output className={styles.code}>{props.code}</output>
        <CopyButton value={props.code} label={props.copyLabel} copiedLabel={props.copiedLabel} />
      </div>
      <Facts items={props.facts} labels="wide" />
      <p className={styles.warning}>{props.warning}</p>
    </section>
  );
}
