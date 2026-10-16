import type { ReactNode } from "react";

import { Brand, Footer, Mark } from "@orchester/ui/components/frame";

import { t } from "@/lib/i18n";
import type { ServerInfo } from "@/lib/model";

import styles from "./page.module.css";

export function AdminBrand() {
  return <Brand name={t("brand.name")} product={t("brand.product")} />;
}

export function AdminMark() {
  return <Mark />;
}

/** The 56 px footer (`D8GGS`): what the server promises, and its version; a phone shows the version only. */
export function AdminFooter({ server }: { server: ServerInfo }) {
  const version = <span>{t("footer.version", { version: server.version, api: server.apiVersion })}</span>;
  return <Footer promise={t("footer.promise", { server: server.name })} aside={version} />;
}

/** The pages before sign-in (`DJUEA`, setup): the brand, what the page asks, the footer. */
export function EntryShell({ server, children }: { server: ServerInfo | null; children: ReactNode }) {
  return (
    <>
      <main className={styles.entry}>
        <AdminBrand />
        {children}
      </main>
      {server !== null && <AdminFooter server={server} />}
    </>
  );
}

/** A card of the pages before sign-in (`bWMCt`): `wide` for the setup's account and the authorize-like pages. */
export function EntryCard({ size, children }: { size?: "wide" | "done"; children: ReactNode }) {
  const width = size === undefined ? "" : ` ${styles[size]}`;
  return <section className={`${styles.card}${width}`}>{children}</section>;
}

/** The sign-in or setup card beside the certificate check (`hxhwb`). */
export function Pair({ children }: { children: ReactNode }) {
  return <div className={styles.pair}>{children}</div>;
}

export function CardHead({ title, description }: { title: string; description?: ReactNode }) {
  return (
    <div className={styles.head}>
      <h1 className={styles.title}>{title}</h1>
      {description !== undefined && <p className={styles.description}>{description}</p>}
    </div>
  );
}

/** Small print in a card (`kiQzB`). */
export function Small({ children }: { children: ReactNode }) {
  return <p className={styles.small}>{children}</p>;
}
