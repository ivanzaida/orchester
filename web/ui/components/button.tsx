import type { ReactNode } from "react";

import { Icon, type IconName } from "./icon";
import styles from "./button.module.css";

/** The file's buttons in their large size: `yJ2jo` primary, `Dswcx` secondary, `cnlv1` ghost; `small` is 28 high. */
export type ButtonKind = "primary" | "secondary" | "ghost";

export function buttonClass(kind: ButtonKind, options: { small?: boolean; busy?: boolean } = {}): string {
  return [styles.button, styles[kind], options.small && styles.small, options.busy && styles.busy]
    .filter(Boolean)
    .join(" ");
}

export function ButtonContent({ icon, spin, children }: { icon?: IconName | undefined; spin?: boolean; children: ReactNode }) {
  return (
    <>
      {icon && <Icon name={icon} size="button" spin={spin ?? false} />}
      <span>{children}</span>
    </>
  );
}

/** A link that looks like a button: it only navigates. */
export function ButtonLink(props: { href: string; kind: ButtonKind; icon?: IconName; children: ReactNode }) {
  return (
    <a className={buttonClass(props.kind)} href={props.href}>
      <ButtonContent icon={props.icon}>{props.children}</ButtonContent>
    </a>
  );
}
