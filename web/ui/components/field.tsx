"use client";

import { useState, type ReactNode } from "react";

import { Icon } from "./icon";
import { useInBrowser } from "../hooks/browser";
import { ErrorLine } from "./status";
import styles from "./field.module.css";

interface FieldProps {
  name: string;
  label: string;
  value: string;
  onChange: (value: string) => void;
  /** The error under the field, in the app's words; it replaces the hint. */
  error?: ReactNode;
  /** `row`: the 32 high field of a list row (`WAGql`); otherwise the 36 high form field. */
  size?: "form" | "row" | undefined;
  hint?: string | undefined;
  /** Beside the label at its right, like *Forgot password?* (`Es2YM`). */
  aside?: ReactNode;
  autoComplete: string;
}

/** The file's `Field/text` (`JFXZg`) at the large size: label, 36 high box, then the error or the hint. */
function Frame(props: FieldProps & { id: string; children: ReactNode }) {
  const label = (
    <label className={styles.label} htmlFor={props.id}>
      {props.label}
    </label>
  );
  return (
    <div className={styles.field}>
      {props.aside ? (
        <div className={styles.labelRow}>
          {label}
          {props.aside}
        </div>
      ) : (
        label
      )}
      <div className={boxClass(props)}>{props.children}</div>
      {/* The hint gives way to its error, as in the app's Choose your password. */}
      {props.error ? (
        <ErrorLine id={`${props.id}-error`}>{props.error}</ErrorLine>
      ) : (
        props.hint && <p id={`${props.id}-hint`} className={styles.hint}>{props.hint}</p>
      )}
    </div>
  );
}

function boxClass(props: FieldProps): string {
  return [styles.box, props.size === "row" && styles.row, Boolean(props.error) && styles.invalid].filter(Boolean).join(" ");
}

function describedBy(props: FieldProps, id: string): string | undefined {
  if (props.error) {
    return `${id}-error`;
  }
  return props.hint ? `${id}-hint` : undefined;
}

export function TextField(props: FieldProps & { type: "email" | "text"; placeholder?: string }) {
  const id = `field-${props.name}`;
  return (
    <Frame {...props} id={id}>
      <input
        id={id}
        className={styles.input}
        name={props.name}
        type={props.type}
        value={props.value}
        placeholder={props.placeholder}
        autoComplete={props.autoComplete}
        aria-invalid={props.error ? true : undefined}
        aria-describedby={describedBy(props, id)}
        onChange={(event) => { props.onChange(event.target.value); }}
      />
    </Frame>
  );
}

/** A password field with the eye that shows the value; the eye appears only where script runs, since only script can use it. */
export function PasswordField(props: FieldProps & { showLabel: string; hideLabel: string }) {
  const id = `field-${props.name}`;
  const [shown, setShown] = useState(false);
  const browser = useInBrowser();
  return (
    <Frame {...props} id={id}>
      <input
        id={id}
        className={`${styles.input}${shown ? "" : ` ${styles.secret}`}`}
        name={props.name}
        type={shown ? "text" : "password"}
        value={props.value}
        autoComplete={props.autoComplete}
        aria-invalid={props.error ? true : undefined}
        aria-describedby={describedBy(props, id)}
        onChange={(event) => { props.onChange(event.target.value); }}
      />
      {browser && (
        <button
          type="button"
          className={styles.reveal}
          aria-label={shown ? props.hideLabel : props.showLabel}
          aria-pressed={shown}
          aria-controls={id}
          onClick={() => { setShown(!shown); }}
        >
          <Icon name={shown ? "eye-off" : "eye"} size="field" />
        </button>
      )}
    </Frame>
  );
}

/** A quiet link beside a field's label (`iqSV1` *Forgot password?*). */
export function AsideLink({ href, children }: { href: string; children: ReactNode }) {
  return (
    <a className={styles.aside} href={href}>
      {children}
    </a>
  );
}
