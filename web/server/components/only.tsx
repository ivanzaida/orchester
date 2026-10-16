import type { ReactNode } from "react";

import styles from "./only.module.css";

/**
 * What a page shows only on a wide window, or only on a phone, where the frames differ (`FCQCr` against `mYBka`).
 * The other one is `display: none`, so it is out of the page for assistive technology too; the shown one adds no box
 * of its own. `inline` for text inside a paragraph.
 */
export function WideOnly({ children, inline = false }: { children: ReactNode; inline?: boolean }) {
  const Tag = inline ? "span" : "div";
  return <Tag className={styles.wide}>{children}</Tag>;
}

export function PhoneOnly({ children, inline = false }: { children: ReactNode; inline?: boolean }) {
  const Tag = inline ? "span" : "div";
  return <Tag className={styles.phone}>{children}</Tag>;
}
