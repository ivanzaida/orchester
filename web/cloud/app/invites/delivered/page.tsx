import type { Metadata } from "next";

import { ButtonLink } from "@orchester/ui/components/button";
import { Actions, Card, Head } from "@orchester/ui/components/card";
import { Shell } from "@/components/page";
import { adminReturn, host } from "@/lib/authorize";
import { t } from "@/lib/i18n";

export const metadata: Metadata = { title: t("invites.deliver.pageTitle") };

const OUTCOMES = {
  sent: { title: "invites.deliver.sentTitle", text: "invites.deliver.sentText", icon: "send", tone: "success" },
  refused: { title: "invites.deliver.refusedTitle", text: "invites.deliver.refusedText", icon: "mail-warning", tone: "warning" },
  unavailable: { title: "invites.deliver.unavailableTitle", text: "invites.deliver.unavailableText", icon: "cloud-off", tone: "warning" },
} as const;

/**
 * How an invite the inviter's browser carried here went (`/invites/deliver`). No frame draws it: it is the Cloud's
 * status card (`WuzmL`) with a way back to the server admin.
 */
export default async function DeliveredPage({ searchParams }: { searchParams: Promise<Record<string, string | string[] | undefined>> }) {
  const params = await searchParams;
  const outcome = typeof params.outcome === "string" && params.outcome in OUTCOMES ? (params.outcome as keyof typeof OUTCOMES) : "refused";
  const shown = OUTCOMES[outcome];
  const back = adminReturn(typeof params.return === "string" ? params.return : "");
  return (
    <Shell>
      <Card>
        <Head title={t(shown.title)} description={t(shown.text)} tile={{ icon: shown.icon, tone: shown.tone }} />
        {back !== null && (
          <Actions>
            <ButtonLink kind="primary" icon="arrow-left" href={back}>
              {t("invites.deliver.back", { place: host(back) })}
            </ButtonLink>
          </Actions>
        )}
      </Card>
    </Shell>
  );
}
