"use client";

import { useState } from "react";

import { useInBrowser } from "../hooks/browser";
import { buttonClass, ButtonContent, type ButtonKind } from "./button";
import { Icon } from "./icon";
import styles from "./copy.module.css";

/**
 * Copies a value to the clipboard (*Copy code*, *Copy address*). Only where script runs, since only script can copy;
 * without it the value stays on the page to select. `iconOnly` is the small copy glyph beside an address (`assq8`).
 */
export function CopyButton(props: { value: string; label: string; copiedLabel: string; kind?: ButtonKind; iconOnly?: boolean }) {
  const browser = useInBrowser();
  const [copied, setCopied] = useState(false);
  if (!browser) {
    return null;
  }
  const copy = () => {
    navigator.clipboard.writeText(props.value).then(
      () => { setCopied(true); },
      // The browser refused (no permission, not a secure page): the value stays on the page to select by hand.
      () => { setCopied(false); },
    );
  };
  const words = copied ? props.copiedLabel : props.label;
  if (props.iconOnly) {
    return (
      <button type="button" className={styles.icon} aria-label={words} title={words} onClick={copy}>
        <Icon name={copied ? "check" : "copy"} size="note" />
      </button>
    );
  }
  return (
    <button type="button" className={buttonClass(props.kind ?? "secondary", { row: true })} onClick={copy}>
      <ButtonContent icon={copied ? "check" : "copy"}>{words}</ButtonContent>
    </button>
  );
}
