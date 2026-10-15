import type { Metadata } from "next";

import { InviteList } from "@/components/invites/invite-list";
import { ProjectInvites } from "@/components/invites/project-invites";
import { MissingProject } from "@/components/projects/missing";
import { ProjectHead } from "@/components/projects/project-head";
import { AdminShell } from "@/components/shell";
import { t } from "@/lib/i18n";
import { loadProject, value } from "@/lib/server/admin";
import { originOf } from "@/lib/server/place";
import { emailReach } from "@/lib/server/reach";
import { invites } from "@/lib/server/team/invites";
import { members } from "@/lib/server/team/projects";
import { formToken } from "@/lib/server/visitor";

type Params = { params: Promise<{ slug: string }> };

export async function generateMetadata({ params }: Params): Promise<Metadata> {
  const { slug } = await params;
  return { title: t("invites.pageTitle", { project: slug }) };
}

/** A project's Invites (`iLFy5`; the code shown once, revoke in place, sent, through the browser in `KagFB`). */
export default async function InvitesPage({ params }: Params) {
  const { slug } = await params;
  const { context, project } = await loadProject(slug);
  if (project === null) {
    return (
      <AdminShell context={context} area="projects">
        <MissingProject server={context.server.name} />
      </AdminShell>
    );
  }
  const [memberList, inviteList] = await Promise.all([members(project.id, context.options), invites(project.id, context.options)]);
  const own = originOf(context.place);
  const csrf = await formToken();
  const { server } = context;
  return (
    <AdminShell context={context} area="projects">
      <ProjectHead project={project} tab="invites" members={value(memberList).length} />
      <ProjectInvites csrf={csrf} server={server.name} project={{ id: project.id, name: project.name, address: project.address }}
        reach={emailReach(server)} back={`${own}/admin/projects/${encodeURIComponent(project.slug)}/invites`}
        checkCode={server.certificate?.checkCode ?? null} />
      <InviteList id="invites" title={t("invites.listTitle")} description={t("invites.listText")} invites={value(inviteList)}
        empty={{ title: t("invites.emptyTitle"), text: t("invites.emptyText", { project: project.name }) }}
        csrf={csrf} server={server.name} now={Date.now()} />
    </AdminShell>
  );
}
