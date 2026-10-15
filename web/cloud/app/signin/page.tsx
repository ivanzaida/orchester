import type { Metadata } from "next";

import { t } from "@/lib/i18n";
import { formToken } from "@/lib/server/visitor";
import { Shell, SwitchLine } from "@/components/page";

import { SignInForm } from "./form";

export const metadata: Metadata = { title: t("signin.pageTitle") };

export default async function SignInPage() {
  return (
    <Shell below={<SwitchLine question={t("signin.question")} link={t("signin.link")} href="/signup" />}>
      <SignInForm csrf={await formToken()} />
    </Shell>
  );
}
