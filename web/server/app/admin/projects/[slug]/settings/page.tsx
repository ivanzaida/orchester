import type { Metadata } from "next";

import { MissingProject } from "@/components/projects/missing";
import { ProjectHead } from "@/components/projects/project-head";
import { Hint, Locked, SettingRow, SettingsPanel, Value } from "@/components/settings-panel";
import { AdminShell } from "@/components/shell";
import { t } from "@/lib/i18n";
import { loadProject, value } from "@/lib/server/admin";
import { members } from "@/lib/server/team/projects";
import { formToken } from "@/lib/server/visitor";

import { SaveField } from "./forms";

type Params = { params: Promise<{ slug: string }> };

export async function generateMetadata({ params }: Params): Promise<Metadata> {
  const { slug } = await params;
  return { title: t("projectSettings.pageTitle", { project: slug }) };
}

/** A project's own settings (`h8OLW5`): its name, its locked address, the repository hint, its owners. */
export default async function ProjectSettingsPage({ params }: Params) {
  const { slug } = await params;
  const { context, project } = await loadProject(slug);
  if (project === null) {
    return (
      <AdminShell context={context} area="projects">
        <MissingProject server={context.server.name} />
      </AdminShell>
    );
  }
  const memberList = value(await members(project.id, context.options));
  const csrf = await formToken();
  return (
    <AdminShell context={context} area="projects">
      <ProjectHead project={project} tab="settings" members={memberList.length} />
      <SettingsPanel>
        <SettingRow label={t("projectSettings.name")}>
          <SaveField csrf={csrf} project={project.id} which="name" label={t("projectSettings.name")} initial={project.name} />
          <Hint>{t("projectSettings.nameHint")}</Hint>
        </SettingRow>
        <SettingRow label={t("projectSettings.address")}>
          <Locked>{project.address}</Locked>
          <Hint>{t("projectSettings.addressHint", { slug: project.slug })}</Hint>
        </SettingRow>
        <SettingRow label={t("projectSettings.hint")}>
          <SaveField csrf={csrf} project={project.id} which="hint" label={t("projectSettings.hint")} initial={project.remoteHint ?? ""} />
          <Hint>{t("projectSettings.hintHelp")}</Hint>
        </SettingRow>
        <SettingRow label={t("projectSettings.owners")}>
          <Value>{project.owners.map((owner) => owner.displayName).join(", ")}</Value>
          <Hint>{t("projectSettings.ownersHint")}</Hint>
        </SettingRow>
      </SettingsPanel>
    </AdminShell>
  );
}
