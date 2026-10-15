"use client";

import { useSyncExternalStore } from "react";

import { t } from "@/lib/i18n";

import styles from "./account.module.css";

export const SECTIONS = ["get-started", "invites", "servers", "devices", "account"] as const;
export type SectionId = (typeof SECTIONS)[number];

function subscribe(changed: () => void): () => void {
  window.addEventListener("hashchange", changed);
  return () => {
    window.removeEventListener("hashchange", changed);
  };
}

function current(): SectionId {
  const hash = window.location.hash.slice(1);
  return SECTIONS.find((id) => id === hash) ?? "get-started";
}

/** Marks a section as the one being worked in, as an action there does (`ZyAsL` marks Account, `wcFhu` Devices). */
export function markSection(id: SectionId): void {
  window.history.replaceState(null, "", `#${id}`);
  window.dispatchEvent(new HashChangeEvent("hashchange"));
}

/** *On this page* (`A2IvUW`): links to the sections, the current one marked; hidden on narrow windows. */
export function AccountNav({ invites }: { invites: number }) {
  const active = useSyncExternalStore(subscribe, current, () => "get-started");
  const labels: Record<SectionId, string> = {
    "get-started": t("account.nav.getStarted"),
    invites: invites > 0 ? t("account.nav.invitesCount", { count: invites }) : t("account.nav.invites"),
    servers: t("account.nav.servers"),
    devices: t("account.nav.devices"),
    account: t("account.nav.account"),
  };
  return (
    <nav className={styles.nav} aria-labelledby="account-nav-title">
      <span id="account-nav-title" className={styles.eyebrow}>
        {t("account.navTitle")}
      </span>
      {SECTIONS.map((id) => (
        <a
          key={id}
          href={`#${id}`}
          className={`${styles.navLink}${active === id ? ` ${styles.current}` : ""}`}
          aria-current={active === id ? "location" : undefined}
        >
          {labels[id]}
        </a>
      ))}
    </nav>
  );
}
