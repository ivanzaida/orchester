"use client";

import type { ChangeEvent } from "react";

import { Icon } from "./icon";
import styles from "./select.module.css";

export interface Option {
  readonly value: string;
  readonly label: string;
}

/**
 * The file's select (`pV2IA`, `M8hrM`), drawn over the browser's own `<select>` so it works with the keyboard and
 * without script; the open list is the browser's. `row`: the 30 high select of a table row; otherwise 32 high.
 */
export function Select(props: {
  name: string;
  label: string;
  options: readonly Option[];
  value?: string;
  defaultValue?: string;
  hideLabel?: boolean;
  size?: "row" | "form";
  disabled?: boolean;
  onChange?: (event: ChangeEvent<HTMLSelectElement>) => void;
}) {
  const id = `select-${props.name}`;
  const control = (
    <span className={`${styles.box}${props.size === "row" ? ` ${styles.row}` : ""}`}>
      <select
        id={id}
        className={styles.select}
        name={props.name}
        value={props.value}
        defaultValue={props.defaultValue}
        disabled={props.disabled}
        aria-label={props.hideLabel ? props.label : undefined}
        onChange={props.onChange}
      >
        {props.options.map((option) => (
          <option key={option.value} value={option.value}>
            {option.label}
          </option>
        ))}
      </select>
      <span className={styles.chevron} aria-hidden="true">
        <Icon name="chevron-down" size="note" />
      </span>
    </span>
  );
  if (props.hideLabel) {
    return control;
  }
  return (
    <span className={styles.field}>
      <label className={styles.label} htmlFor={id}>
        {props.label}
      </label>
      {control}
    </span>
  );
}
