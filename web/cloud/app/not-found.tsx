import { t } from "@/lib/i18n";
import { ButtonLink } from "@orchester/ui/components/button";
import { Card, Head } from "@orchester/ui/components/card";
import { Shell } from "@/components/page";

export default function NotFound() {
  return (
    <Shell>
      <Card>
        <Head title={t("failure.missingTitle")} description={t("failure.missingDescription")} />
        <ButtonLink kind="secondary" icon="log-in" href="/signin">
          {t("failure.signIn")}
        </ButtonLink>
      </Card>
    </Shell>
  );
}
