"use client";

import { buttonClass, ButtonContent } from "@orchester/ui/components/button";
import { Card, Head } from "@orchester/ui/components/card";
import { CenteredPage } from "@orchester/ui/components/frame";

import { AdminBrand } from "@/components/page";
import { t } from "@/lib/i18n";

/** A page the server could not answer for: said plainly, with a way to try again. Details stay in the server's log. */
export default function Failure({ retry }: { retry: () => void }) {
  return (
    <CenteredPage brand={<AdminBrand />} footer={null}>
      <Card>
        <Head title={t("failure.title")} description={t("failure.description")} tile={{ icon: "circle-alert", tone: "warning" }} />
        <button type="button" className={buttonClass("secondary")} onClick={() => { retry(); }}>
          <ButtonContent icon="refresh-cw">{t("failure.retry")}</ButtonContent>
        </button>
      </Card>
    </CenteredPage>
  );
}
