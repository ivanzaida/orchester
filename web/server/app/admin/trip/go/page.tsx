import type { Metadata } from "next";
import { redirect } from "next/navigation";

import { Carry } from "@/components/carry";
import { CardHead, EntryCard, EntryShell } from "@/components/page";
import { t } from "@/lib/i18n";
import { handoff } from "@/lib/server/visitor";

export const metadata: Metadata = { title: t("trip.pageTitle") };

/**
 * Between the trip route and Orchester Cloud: the page posts the trip (its fields sealed in a short-lived cookie by
 * the route) to the cloud's `/authorize/start`. No frame draws it; it is the sign-in card with one button, shown only
 * while the browser leaves, or to a browser without script.
 */
export default async function TripGo() {
  const trip = await handoff();
  if (trip === null) {
    redirect("/admin/signin");
  }
  return (
    <EntryShell server={null}>
      <EntryCard>
        <CardHead title={t("trip.title")} description={t("trip.text")} />
        <Carry action={trip.action} fields={trip.fields} />
      </EntryCard>
    </EntryShell>
  );
}
