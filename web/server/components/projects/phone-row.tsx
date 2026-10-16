import { Icon } from "@orchester/ui/components/icon";
import { AddressLine, Chip, TopLine } from "@orchester/ui/components/list";

import { bareAddress } from "@/lib/codes";
import { t } from "@/lib/i18n";
import type { Project } from "@/lib/model";

import { memberCount } from "./project-row";
import styles from "./phone.module.css";

/**
 * A project on a phone (`Gk7u7` as `mYBka` uses it): no glyph tile and no repository hint; its members on the third
 * line with the people glyph, and the chevron at the right. The whole row opens its Members.
 */
export function PhoneProjectRow({ project }: { project: Project }) {
  return (
    <a className={styles.link} href={`/admin/projects/${encodeURIComponent(project.slug)}`}>
      <span className={styles.text}>
        <TopLine name={project.name} weight="semibold">
          {project.role === "owner" && <Chip>{t("projects.owned")}</Chip>}
        </TopLine>
        <AddressLine>{bareAddress(project.address)}</AddressLine>
        <span className={styles.members}>
          <Icon name="users" size="reason" />
          <span>{memberCount(project)}</span>
        </span>
      </span>
      <span className={styles.chevron}>
        <Icon name="chevron-right" size="glyph" />
      </span>
    </a>
  );
}
