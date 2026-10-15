import type { Metadata } from "next";

import { ButtonLink } from "@orchester/ui/components/button";
import { EmptyState, Panel } from "@orchester/ui/components/list";

import { Note, PageHead } from "@/components/page-head";
import { ProjectRow } from "@/components/projects/project-row";
import { WaitingCallout } from "@/components/projects/waiting";
import { AdminShell } from "@/components/shell";
import { t } from "@/lib/i18n";
import type { Approval } from "@/lib/model";
import { loadAdmin, type AdminContext } from "@/lib/server/admin";
import { approvals } from "@/lib/server/team/auth";

import styles from "./projects.module.css";

export const metadata: Metadata = { title: t("projects.pageTitle") };

/** The computers waiting for join approval, for admins; a failure here only leaves the callout out. */
async function waiting(context: AdminContext): Promise<readonly Approval[]> {
  if (!context.me.admin) {
    return [];
  }
  const result = await approvals(context.options);
  return result.ok ? result.value : [];
}

/**
 * Projects (`FCQCr`; a phone `mYBka`; an owner who isn't an admin and none yet in `Gs9Vu`, `KagFB`): every project
 * for admins, the projects they own for owners.
 */
export default async function ProjectsPage() {
  const context = await loadAdmin();
  const { server, me, projects } = context;
  const now = Date.now();
  const newProject = me.admin ? (
    <ButtonLink kind="primary" row icon="plus" href="/admin/projects/new">
      {t("projects.new")}
    </ButtonLink>
  ) : undefined;
  return (
    <AdminShell context={context} area="projects">
      <PageHead title={t("projects.title")} description={me.admin ? t("projects.description") : t("projects.descriptionOwner")}
        actions={newProject} />
      <WaitingCallout approvals={await waiting(context)} />
      <div className={styles.list}>
        <Panel>
          {projects.length === 0 ? (
            <div className={styles.empty}>
              <EmptyState icon="folder-plus" title={t("projects.emptyTitle")}
                text={me.admin ? t("projects.emptyText") : t("projects.emptyOwnerText", { server: server.name })} />
              {newProject}
            </div>
          ) : (
            projects.map((project) => <ProjectRow key={project.id} project={project} now={now} />)
          )}
        </Panel>
        {me.admin && <Note>{t("projects.noteAdmin", { server: server.name })}</Note>}
      </div>
    </AdminShell>
  );
}
