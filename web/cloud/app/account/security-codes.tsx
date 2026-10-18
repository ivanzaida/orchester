"use client";

import { ActionButton, ButtonContent, buttonClass } from "@orchester/ui/components/button";
import { CopyButton } from "@orchester/ui/components/copy";
import { Icon } from "@orchester/ui/components/icon";
import { Badge } from "@orchester/ui/components/list";

import { t } from "@/lib/i18n";
import { recoveryCodesText } from "@/lib/two-factor";

import styles from "./security.module.css";

/**
 * Recovery codes shown once, after turning two-factor authentication on or making new ones: the codes, *Copy codes*,
 * *Download* (a text file made in the page, one code a line) and *Done*. They exist only in this page's state; the
 * cloud keeps only their HMAC.
 */
export function CodesShown({ codes, onDone }: { codes: readonly string[]; onDone: () => void }) {
  const text = recoveryCodesText(codes);
  return (
    <div className={styles.flow} role="status">
      <span className={styles.head}>
        <Icon name="key-round" size="glyph" />
        <span className={styles.title}>{t("account.security.codesTitle")}</span>
        <Badge tone="warning">{t("account.security.codesBadge")}</Badge>
      </span>
      <p className={styles.text}>{t("account.security.codesText")}</p>
      <ol className={styles.codes} aria-label={t("account.security.codesTitle")}>
        {codes.map((code) => (
          <li key={code}>{code}</li>
        ))}
      </ol>
      <span className={styles.buttons}>
        <CopyButton value={text} label={t("account.security.copyCodes")} copiedLabel={t("account.security.copiedCodes")} />
        <a className={buttonClass("secondary", { row: true })} href={`data:text/plain;charset=utf-8,${encodeURIComponent(text)}`}
          download={t("account.security.downloadName")}>
          <ButtonContent icon="download">{t("account.security.download")}</ButtonContent>
        </a>
        <ActionButton kind="primary" row icon="check" onClick={onDone}>
          {t("account.security.done")}
        </ActionButton>
      </span>
    </div>
  );
}
