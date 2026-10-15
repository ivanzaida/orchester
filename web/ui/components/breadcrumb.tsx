import { Icon } from "./icon";
import styles from "./breadcrumb.module.css";

/** Where a page sits (`cRZ9N`): links up, then the page itself. */
export function Breadcrumb({ label, trail, here }: { label: string; trail: readonly { href: string; label: string }[]; here: string }) {
  return (
    <nav className={styles.breadcrumb} aria-label={label}>
      {trail.map((step) => (
        <span key={step.href} className={styles.step}>
          <a className={styles.link} href={step.href}>
            {step.label}
          </a>
          <Icon name="chevron-right" size="reason" />
        </span>
      ))}
      <span className={styles.here} aria-current="page">
        {here}
      </span>
    </nav>
  );
}
