import type { Metadata } from "next";

import { t } from "@/lib/i18n";
import { formToken } from "@/lib/server/visitor";
import { Shell, SwitchLine } from "@/ui/shell";

import { SignUpForm } from "./form";

export const metadata: Metadata = { title: t("signup.pageTitle") };

export default async function SignUpPage() {
  return (
    <Shell below={<SwitchLine question={t("signup.question")} link={t("signup.link")} href="/signin" />}>
      <SignUpForm csrf={await formToken()} />
    </Shell>
  );
}
