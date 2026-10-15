"use client";

import { t } from "@/lib/i18n";
import { buttonClass, ButtonContent } from "@orchester/ui/components/button";
import { Card, Head } from "@orchester/ui/components/card";
import { Shell } from "@/components/page";

/** A page the cloud could not answer for: said plainly, with a way to try again. Details stay in the server's log. */
export default function Failure({ retry }: { retry: () => void }) {
  return (
    <Shell>
      <Card>
        <Head title={t("failure.title")} description={t("failure.description")} tile={{ icon: "circle-alert", tone: "warning" }} />
        <button type="button" className={buttonClass("secondary")} onClick={() => { retry(); }}>
          <ButtonContent icon="refresh-cw">{t("failure.retry")}</ButtonContent>
        </button>
      </Card>
    </Shell>
  );
}
