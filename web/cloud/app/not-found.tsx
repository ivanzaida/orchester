import { t } from "@/lib/i18n";
import { ButtonLink } from "@/ui/button";
import { Card, Head } from "@/ui/card";
import { Shell } from "@/ui/shell";

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
