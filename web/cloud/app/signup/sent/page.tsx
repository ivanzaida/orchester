import type { Metadata } from "next";
import { redirect } from "next/navigation";

import { t } from "@/lib/i18n";
import { RESEND_COOLDOWN_MS } from "@/lib/resend";
import { formToken, signupFlow } from "@/lib/server/visitor";
import { Address, Body, Card, Head, Small, Text, Top } from "@/ui/card";
import { Shell, SwitchLine } from "@/ui/shell";

import { ResendPanel } from "./resend";

export const metadata: Metadata = { title: t("sent.pageTitle") };

/** *Check your email* after sign-up (`y4FZY`; sent again `o62WH`, limit `PLwGT`). */
export default async function SentPage() {
  const flow = await signupFlow();
  if (flow === null) {
    redirect("/signup");
  }
  const now = Date.now();
  const limitedUntil = flow.limitedUntil !== undefined && flow.limitedUntil > now ? flow.limitedUntil : undefined;
  return (
    <Shell below={<SwitchLine question={t("sent.question")} link={t("sent.link")} href="/signin" />}>
      <Card>
        <Top>
          <Head title={t("sent.title")} description={t("sent.description")} tile={{ icon: "mail", tone: "neutral" }} />
          <Address email={flow.email} />
        </Top>
        <Body>
          <Text>{t("sent.next")}</Text>
          <Small>{t("sent.registered")}</Small>
        </Body>
        <ResendPanel
          key={`${flow.sentAt}-${flow.limitedUntil ?? 0}`}
          csrf={await formToken()}
          resentAt={flow.resent ? flow.sentAt : undefined}
          cooldownMs={Math.max(0, flow.sentAt + RESEND_COOLDOWN_MS - now)}
          limit={limitedUntil === undefined ? undefined : { until: limitedUntil, ms: limitedUntil - now }}
        />
      </Card>
    </Shell>
  );
}
