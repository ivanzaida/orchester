import type { Metadata } from "next";
import { redirect } from "next/navigation";

import { t } from "@/lib/i18n";
import { forgotFlow } from "@/lib/server/visitor";
import { ButtonLink } from "@/ui/button";
import { Address, Body, ButtonRow, Card, Head, Small, Text, Top } from "@/ui/card";
import { Shell } from "@/ui/shell";

export const metadata: Metadata = { title: t("forgot.sentTitle") };

/** *Check your email* after *Send reset link*: the same words whether or not the address has an account (`v5jUp6`). */
export default async function ForgotSentPage() {
  const flow = await forgotFlow();
  if (flow === null) {
    redirect("/forgot");
  }
  return (
    <Shell>
      <Card>
        <Top>
          <Head title={t("forgot.sentTitle")} description={t("forgot.sentDescription")} tile={{ icon: "mail", tone: "neutral" }} />
          <Address email={flow.email} />
        </Top>
        <Body>
          <Text>{t("forgot.sentNext")}</Text>
          <Small>{t("forgot.sentSmall")}</Small>
        </Body>
        <ButtonRow>
          <ButtonLink kind="secondary" icon="arrow-left" href="/signin">
            {t("forgot.back")}
          </ButtonLink>
          <ButtonLink kind="ghost" href="/forgot">
            {t("forgot.useAnother")}
          </ButtonLink>
        </ButtonRow>
      </Card>
    </Shell>
  );
}
