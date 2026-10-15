import type { Metadata } from "next";

import { Note } from "@/components/page-head";
import { MissingProject } from "@/components/projects/missing";
import { ProjectHead } from "@/components/projects/project-head";
import { AdminShell } from "@/components/shell";
import { t } from "@/lib/i18n";
import { loadProject, value } from "@/lib/server/admin";
import { members } from "@/lib/server/team/projects";
import { formToken } from "@/lib/server/visitor";

import { MembersTable } from "./members";
import styles from "./members.module.css";

type Params = { params: Promise<{ slug: string }> };

export async function generateMetadata({ params }: Params): Promise<Metadata> {
  const { slug } = await params;
  return { title: t("members.pageTitle", { project: slug }) };
}

/** A project's Members (`Mhudf`): roles, removing, and *Leave* for oneself. */
export default async function MembersPage({ params }: Params) {
  const { slug } = await params;
  const { context, project } = await loadProject(slug);
  if (project === null) {
    return (
      <AdminShell context={context} area="projects">
        <MissingProject server={context.server.name} />
      </AdminShell>
    );
  }
  const list = value(await members(project.id, context.options));
  const words = { project: project.name, server: context.server.name };
  return (
    <AdminShell context={context} area="projects">
      <ProjectHead project={project} tab="members" members={list.length} />
      <div className={styles.stack}>
        <p className={styles.lead}>{t("members.description", words)}</p>
        <MembersTable members={list} me={context.me.id} project={{ id: project.id, name: project.name }}
          server={context.server.name} csrf={await formToken()} now={Date.now()} />
        <Note>{t("members.note", words)}</Note>
      </div>
    </AdminShell>
  );
}
