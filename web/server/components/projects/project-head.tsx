import { Breadcrumb } from "@orchester/ui/components/breadcrumb";
import { ButtonLink } from "@orchester/ui/components/button";
import { CopyButton } from "@orchester/ui/components/copy";
import { Icon } from "@orchester/ui/components/icon";
import { Tabs } from "@orchester/ui/components/tabs";

import { PageHead } from "@/components/page-head";
import { t } from "@/lib/i18n";
import type { Project } from "@/lib/model";

import styles from "./project-head.module.css";

export type ProjectTab = "members" | "invites" | "settings";

/** The address with its copy glyph, and the repository hint (`lOlca`). */
function AddressLine({ project }: { project: Project }) {
  return (
    <p className={styles.address}>
      <span className={styles.url}>{project.address}</span>
      <CopyButton value={project.address} label={t("project.copyAddress")} copiedLabel={t("project.copiedAddress")} iconOnly />
      {project.remoteHint !== null && (
        <>
          <span aria-hidden="true">·</span>
          <span className={styles.hint}>
            <Icon name="git-branch" size="note" />
            <span>{project.remoteHint}</span>
          </span>
        </>
      )}
    </p>
  );
}

/** A project's pages share their head (`Mhudf`): where they are, the name and address, *Invite people*, the tabs. */
export function ProjectHead({ project, tab, members }: { project: Project; tab: ProjectTab; members: number }) {
  const base = `/admin/projects/${encodeURIComponent(project.slug)}`;
  const invite = tab === "members" ? (
    <ButtonLink kind="primary" row icon="user-plus" href={`${base}/invites`}>
      {t("project.invitePeople")}
    </ButtonLink>
  ) : undefined;
  return (
    <div className={styles.head}>
      <Breadcrumb label={t("common.where")} trail={[{ href: "/admin", label: t("projects.title") }]} here={project.name} />
      <PageHead title={project.name} under={<AddressLine project={project} />} actions={invite} />
      <Tabs
        label={t("project.tabs")}
        items={[
          { href: base, label: t("project.tabMembers", { count: members }), current: tab === "members" },
          { href: `${base}/invites`, label: t("project.tabInvites", { count: project.openInvites }), current: tab === "invites" },
          { href: `${base}/settings`, label: t("project.tabProject"), current: tab === "settings" },
        ]}
      />
    </div>
  );
}
