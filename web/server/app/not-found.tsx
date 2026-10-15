import { ButtonLink } from "@orchester/ui/components/button";
import { Card, Head } from "@orchester/ui/components/card";
import { CenteredPage } from "@orchester/ui/components/frame";

import { AdminBrand } from "@/components/page";
import { t } from "@/lib/i18n";

export default function NotFound() {
  return (
    <CenteredPage brand={<AdminBrand />} footer={null}>
      <Card>
        <Head title={t("failure.missingTitle")} description={t("failure.missingDescription")} />
        <ButtonLink kind="secondary" href="/admin">
          {t("failure.back")}
        </ButtonLink>
      </Card>
    </CenteredPage>
  );
}
