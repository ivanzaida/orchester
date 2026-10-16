import { Icon } from "@orchester/ui/components/icon";
import { AddressLine, Chip, Row, RowLine, TopLine } from "@orchester/ui/components/list";

import { Phrase } from "@/components/when";
import { bareAddress } from "@/lib/codes";
import { t } from "@/lib/i18n";
import type { Project } from "@/lib/model";

import styles from "./projects.module.css";

export function memberCount(project: Project): string {
  const members = project.members === 1 ? t("projects.membersOne") : t("projects.members", { count: project.members });
  if (project.openInvites === 0) {
    return members;
  }
  const invites = project.openInvites === 1 ? t("projects.invitesOne") : t("projects.invites", { count: project.openInvites });
  return t("projects.facts", { members, invites });
}

/** The repository hint with its branch glyph (`EVt8e`), or that there is none yet. */
export function HintLine({ hint }: { hint: string | null }) {
  return (
    <span className={styles.hint}>
      <Icon name="git-branch" size="reason" />
      <span>{hint ?? t("projects.noHint")}</span>
    </span>
  );
}

/** A project in the list (`Gk7u7`): the whole row opens its Members. */
export function ProjectRow({ project, now }: { project: Project; now: number }) {
  const side = (
    <span className={styles.side}>
      <span className={styles.facts}>
        <RowLine tone="secondary" wide>
          {memberCount(project)}
        </RowLine>
        <span className={styles.created}>
          <RowLine>
            <Phrase k="projects.created" params={{ name: project.createdBy.displayName }} dates={{ date: { at: project.createdAt, format: "date" } }} now={now} />
          </RowLine>
        </span>
      </span>
      <span className={styles.chevron}>
        <Icon name="chevron-right" size="glyph" />
      </span>
    </span>
  );
  return (
    <a className={styles.link} href={`/admin/projects/${encodeURIComponent(project.slug)}`}>
      <Row glyph="folder-git-2" align="center" side={side}>
        <TopLine name={project.name} weight="semibold">
          {project.role === "owner" && <Chip>{t("projects.owned")}</Chip>}
        </TopLine>
        <AddressLine>{bareAddress(project.address)}</AddressLine>
        <HintLine hint={project.remoteHint} />
      </Row>
    </a>
  );
}
