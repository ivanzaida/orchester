import type { Metadata } from "next";

import { nextAfterSignIn } from "@/lib/authorize";
import { t } from "@/lib/i18n";
import { formToken } from "@/lib/server/visitor";
import { Shell, SwitchLine } from "@/components/page";

import { SignInForm } from "./form";

export const metadata: Metadata = { title: t("signin.pageTitle") };

/** `next`: where to go after signing in, only back to `/authorize` (a team server's admin asking who this is). */
export default async function SignInPage({ searchParams }: { searchParams: Promise<{ next?: string | string[] }> }) {
  const { next } = await searchParams;
  const after = nextAfterSignIn(typeof next === "string" ? next : undefined);
  return (
    <Shell below={<SwitchLine question={t("signin.question")} link={t("signin.link")} href="/signup" />}>
      <SignInForm csrf={await formToken()} next={after ?? undefined} />
    </Shell>
  );
}
