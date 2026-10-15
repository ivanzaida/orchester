import type { Metadata } from "next";

import { t } from "@/lib/i18n";
import { formToken } from "@/lib/server/visitor";
import { Shell, SwitchLine } from "@/ui/shell";

import { ForgotForm } from "./form";

export const metadata: Metadata = { title: t("forgot.pageTitle") };

export default async function ForgotPage() {
  return (
    <Shell below={<SwitchLine question={t("forgot.question")} link={t("forgot.link")} href="/signin" />}>
      <ForgotForm csrf={await formToken()} />
    </Shell>
  );
}
