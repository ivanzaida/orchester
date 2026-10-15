import styles from "./tabs.module.css";

export interface TabLink {
  readonly href: string;
  readonly label: string;
  readonly current: boolean;
}

/** Page tabs (`loUWZ` active, `NspMt` default) as links: each tab is its own address. */
export function Tabs({ label, items }: { label: string; items: readonly TabLink[] }) {
  return (
    <nav className={styles.tabs} aria-label={label}>
      {items.map((item) => (
        <a
          key={item.href}
          href={item.href}
          className={`${styles.tab}${item.current ? ` ${styles.current}` : ""}`}
          aria-current={item.current ? "page" : undefined}
        >
          {item.label}
        </a>
      ))}
    </nav>
  );
}
