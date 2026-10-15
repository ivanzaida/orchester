import type { Metadata } from "next";

import { t } from "@/lib/i18n";
import { linkEmail } from "@/lib/server/link";
import { formToken } from "@/lib/server/visitor";
import { Shell } from "@/ui/shell";

import { ConfirmViews } from "../views";

export const metadata: Metadata = { title: t("confirm.pageTitle") };

/** Where the confirmation email's link lands. Opening it confirms nothing; the visitor presses *Confirm email*. */
export default async function ConfirmPage({ params }: { params: Promise<{ token: string }> }) {
  const { token } = await params;
  const email = await linkEmail(token, "confirm");
  const csrf = await formToken();
  return (
    <Shell>
      <ConfirmViews
        csrf={csrf}
        token={token}
        initial={email === null ? { view: "expired" } : { view: "confirm", email }}
      />
    </Shell>
  );
}
