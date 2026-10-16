import { Avatar } from "@orchester/ui/components/avatar";

import { initials } from "@/lib/codes";
import { t } from "@/lib/i18n";

import styles from "./person.module.css";

/** The avatar, the name (*· you* for oneself) and the username (`a77drO`), in member and people tables. */
export function Person({ name, username, self }: { name: string; username: string; self: boolean }) {
  return (
    <span className={styles.person}>
      <Avatar initials={initials(name)} size="row" />
      <span className={styles.identity}>
        <span className={styles.nameLine}>
          <span className={styles.name}>{name}</span>
          {self && <span className={styles.you}>{t("common.you")}</span>}
        </span>
        <span className={styles.username}>{username}</span>
      </span>
    </span>
  );
}
