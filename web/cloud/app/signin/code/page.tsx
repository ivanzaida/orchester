import type { Metadata } from "next";

import { Shell, SwitchLine } from "@/components/page";
import { t } from "@/lib/i18n";
import { waitingChallenge } from "@/lib/server/challenge";
import { formToken } from "@/lib/server/visitor";

import { CodeForm } from "./form";

export const metadata: Metadata = { title: t("signin.code.pageTitle") };

/**
 * The second step of a sign-in with two-factor authentication. Without a challenge waiting in this browser (none, or
 * past its five minutes), it says to sign in again (`CodeForm`).
 */
export default async function SignInCodePage() {
  const waiting = await waitingChallenge();
  const below = <SwitchLine question={t("signin.code.question")} link={t("signin.code.link")} href="/signin" />;
  return (
    <Shell below={below}>
      <CodeForm csrf={await formToken()} email={waiting?.email ?? null} />
    </Shell>
  );
}
