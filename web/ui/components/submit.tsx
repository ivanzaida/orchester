"use client";

import { useFormStatus } from "react-dom";

import { buttonClass, ButtonContent, type ButtonKind } from "./button";
import type { IconName } from "./icon";

/**
 * A form's submit button. While its form is being sent it is dimmed (`--opacity-busy`) with a turning loader and the
 * busy label (`Sa0WY`, *Creating account…*); `unavailable` dims it further and stops presses (`Rz2FC`).
 */
export function Submit(props: {
  kind: ButtonKind;
  icon?: IconName;
  label: string;
  busyLabel: string;
  unavailable?: boolean;
  small?: boolean;
  row?: boolean;
  describedBy?: string;
  /** The id of the form it sends when it sits outside it, e.g. two forms' buttons stacked together. */
  form?: string;
  /** For forms that start their action themselves (after checking the fields), where `useFormStatus` sees nothing. */
  pending?: boolean;
}) {
  const status = useFormStatus();
  const pending = status.pending || (props.pending ?? false);
  return (
    <button
      type="submit"
      className={buttonClass(props.kind, { small: props.small ?? false, row: props.row ?? false, busy: pending })}
      disabled={pending || (props.unavailable ?? false)}
      aria-describedby={props.describedBy}
      form={props.form}
    >
      <ButtonContent icon={pending ? "loader-circle" : props.icon} spin={pending}>
        {pending ? props.busyLabel : props.label}
      </ButtonContent>
    </button>
  );
}
