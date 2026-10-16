import type { Metadata } from "next";
import { randomUUID } from "node:crypto";

import { Breadcrumb } from "@orchester/ui/components/breadcrumb";
import { Callout } from "@orchester/ui/components/callout";

import { Note, PageHead } from "@/components/page-head";
import { AdminShell } from "@/components/shell";
import { t } from "@/lib/i18n";
import { loadAdmin } from "@/lib/server/admin";
import { originOf } from "@/lib/server/place";
import { formToken } from "@/lib/server/visitor";

import { NewProjectForm } from "./form";
import styles from "./new.module.css";

export const metadata: Metadata = { title: t("newProject.pageTitle") };

/** New project (`gv7xE`): admins create projects by name and slug; they own it. */
export default async function NewProjectPage() {
  const context = await loadAdmin();
  const { server, me } = context;
  return (
    <AdminShell context={context} area="projects">
      <Breadcrumb label={t("common.where")} trail={[{ href: "/admin", label: t("projects.title") }]} here={t("newProject.title")} />
      <PageHead title={t("newProject.title")} description={t("newProject.description", { server: server.name })} />
      {me.admin ? (
        <NewProjectForm csrf={await formToken()} idempotencyKey={randomUUID()} prefix={`${originOf(context.place)}/`} owner={me.displayName} />
      ) : (
        <Callout tone="neutral" icon="lock" title={t("newProject.adminsOnly", { server: server.name })} />
      )}
      <div className={styles.note}>
        <Note>{t("newProject.note")}</Note>
      </div>
    </AdminShell>
  );
}
