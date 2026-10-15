import type { ReactNode } from "react";

import { Icon, type IconName } from "./icon";
import styles from "./button.module.css";

/** The file's buttons in their large size: `yJ2jo` primary, `Dswcx` secondary, `cnlv1` ghost; `small` is 28 high. */
export type ButtonKind = "primary" | "secondary" | "ghost";

/**
 * `row`: the 32 high button of a list row (`J2YICE`), as wide as its label; `inline`: 36 high and as wide as its label
 * (`yUHZq`); `small`: 28 high (`p506N`). Otherwise a form's full-width 36 high button.
 */
export interface ButtonSize {
  small?: boolean;
  row?: boolean;
  inline?: boolean;
  busy?: boolean;
}

export function buttonClass(kind: ButtonKind, options: ButtonSize = {}): string {
  const size = [options.small && styles.small, options.row && styles.row, options.inline && styles.inline];
  return [styles.button, styles[kind], ...size, options.busy && styles.busy]
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
export function ButtonLink(props: { href: string; kind: ButtonKind; icon?: IconName; row?: boolean; children: ReactNode }) {
  return (
    <a className={buttonClass(props.kind, { row: props.row ?? false })} href={props.href}>
      <ButtonContent icon={props.icon}>{props.children}</ButtonContent>
    </a>
  );
}

/** A button that acts in the page (opens a form, cancels it), or that cannot be pressed now (`unavailable`). */
export function ActionButton(props: {
  kind: ButtonKind;
  icon?: IconName;
  row?: boolean;
  inline?: boolean;
  unavailable?: boolean;
  describedBy?: string;
  onClick?: () => void;
  children: ReactNode;
}) {
  return (
    <button
      type="button"
      className={buttonClass(props.kind, { row: props.row ?? false, inline: props.inline ?? false })}
      disabled={props.unavailable ?? false}
      aria-describedby={props.describedBy}
      onClick={props.onClick}
    >
      <ButtonContent icon={props.icon}>{props.children}</ButtonContent>
    </button>
  );
}
