import { ButtonLink } from "@orchester/ui/components/button";
import { EmptyState, Panel } from "@orchester/ui/components/list";

import { t } from "@/lib/i18n";

/** A project address under `/admin/projects/` that names no project this person manages. */
export function MissingProject({ server }: { server: string }) {
  return (
    <Panel>
      <EmptyState icon="folder-git-2" title={t("project.missingTitle")} text={t("project.missingText", { server })} />
      <ButtonLink kind="secondary" href="/admin">
        {t("project.back")}
      </ButtonLink>
    </Panel>
  );
}
