import type { ReactNode } from "react";

import { t } from "@/lib/i18n";

import styles from "./shell.module.css";

/** The three bars of `OLHng` *Brand / Orchester* in its raised tile, then the wordmark and the CLOUD tag (`V4PyD`). */
function Brand() {
  return (
    <div className={styles.brand}>
      <span className={styles.mark} aria-hidden="true">
        <span className={styles.logo}>
          <span className={styles.bar1} />
          <span className={styles.bar2} />
          <span className={styles.bar3} />
        </span>
      </span>
      <span className={styles.wordmark}>{t("brand.name")}</span>
      <span className={styles.tag}>{t("brand.product")}</span>
    </div>
  );
}

function Footer() {
  return (
    <footer className={styles.footer}>
      <p className={styles.promise}>{t("footer.promise")}</p>
      <span className={styles.spacer} />
      <span className={styles.links}>
        <span>{t("footer.terms")}</span>
        <span>{t("footer.privacy")}</span>
      </span>
    </footer>
  );
}

/** Every page: the brand, one card and an optional line under it in a centred column, then the footer (`AmM6K`). */
export function Shell({ children, below }: { children: ReactNode; below?: ReactNode }) {
  return (
    <>
      <main className={styles.main}>
        <Brand />
        {children}
        {below}
      </main>
      <Footer />
    </>
  );
}

/** *Already have an account? Sign in* under the card (`VBBaE`). */
export function SwitchLine({ question, link, href }: { question: string; link: string; href: string }) {
  return (
    <p className={styles.switch}>
      <span>{question}</span>
      <a className={styles.link} href={href}>
        {link}
      </a>
    </p>
  );
}
