import type { Metadata } from "next";

import { ButtonLink } from "@orchester/ui/components/button";
import { EmptyState, Panel } from "@orchester/ui/components/list";

import { PhoneOnly, WideOnly } from "@/components/only";
import { Note, PageHead } from "@/components/page-head";
import { PhoneProjectRow } from "@/components/projects/phone-row";
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

/** An admin's description: the full one, or the short one a phone draws (`mYBka`). */
function adminDescription() {
  return (
    <>
      <WideOnly inline>{t("projects.description")}</WideOnly>
      <PhoneOnly inline>{t("projects.descriptionPhone")}</PhoneOnly>
    </>
  );
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
      <PageHead title={t("projects.title")} description={me.admin ? adminDescription() : t("projects.descriptionOwner")}
        actions={newProject} />
      <WaitingCallout approvals={await waiting(context)} />
      <div className={styles.list}>
        {projects.length === 0 ? (
          <Panel>
            <div className={styles.empty}>
              <EmptyState icon="folder-plus" title={t("projects.emptyTitle")}
                text={me.admin ? t("projects.emptyText") : t("projects.emptyOwnerText", { server: server.name })} />
              {newProject}
            </div>
          </Panel>
        ) : (
          <>
            <WideOnly>
              <Panel>{projects.map((project) => <ProjectRow key={project.id} project={project} now={now} />)}</Panel>
            </WideOnly>
            <PhoneOnly>
              <Panel>{projects.map((project) => <PhoneProjectRow key={project.id} project={project} />)}</Panel>
            </PhoneOnly>
          </>
        )}
        {me.admin && (
          <WideOnly>
            <Note>{t("projects.noteAdmin", { server: server.name })}</Note>
          </WideOnly>
        )}
        <PhoneOnly>
          <Note>{t("projects.notePhone")}</Note>
        </PhoneOnly>
      </div>
    </AdminShell>
  );
}
