import type { Metadata } from "next";

import { ButtonLink } from "@orchester/ui/components/button";
import { Actions, Card, Head } from "@orchester/ui/components/card";
import { Shell } from "@/components/page";
import { host } from "@/lib/authorize";
import { delivered } from "@/lib/server/visitor";
import { t } from "@/lib/i18n";

export const metadata: Metadata = { title: t("invites.deliver.pageTitle") };

const OUTCOMES = {
  sent: { title: "invites.deliver.sentTitle", text: "invites.deliver.sentText", icon: "send", tone: "success" },
  refused: { title: "invites.deliver.refusedTitle", text: "invites.deliver.refusedText", icon: "mail-warning", tone: "warning" },
  unavailable: { title: "invites.deliver.unavailableTitle", text: "invites.deliver.unavailableText", icon: "cloud-off", tone: "warning" },
} as const;

/**
 * How an invite the inviter's browser carried here went (`/invites/deliver`). No frame draws it: it is the Cloud's
 * status card (`WuzmL`) with a way back to the admin of the server the cloud registered for the invite. The page takes
 * nothing from its address (security review 1, minor 2).
 */
export default async function DeliveredPage() {
  const flow = await delivered();
  const shown = OUTCOMES[flow?.outcome ?? "refused"];
  const back = flow?.back ?? null;
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
