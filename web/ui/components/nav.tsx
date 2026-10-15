import styles from "./nav.module.css";

export interface NavLink {
  readonly href: string;
  readonly label: string;
  readonly current: boolean;
}

/** A page's left link list (`EJ8tl`, links `xWDPH` and `v4rnYw`): a small mono title, then the links. */
export function SideNav({ title, items }: { title: string; items: readonly NavLink[] }) {
  return (
    <nav className={styles.nav} aria-label={title}>
      <span className={styles.eyebrow}>{title}</span>
      {items.map((item) => (
        <a
          key={item.href}
          href={item.href}
          className={`${styles.link}${item.current ? ` ${styles.current}` : ""}`}
          aria-current={item.current ? "page" : undefined}
        >
          {item.label}
        </a>
      ))}
    </nav>
  );
}
