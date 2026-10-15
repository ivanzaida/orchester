import type { Metadata } from "next";

import { t } from "@/lib/i18n";
import { inspect } from "@/lib/server/link";
import { formToken } from "@/lib/server/visitor";
import { Shell } from "@/components/page";

import { ConfirmViews } from "../views";

export const metadata: Metadata = { title: t("confirm.pageTitle") };

/** Where the confirmation email's link lands. Opening it confirms nothing; the visitor presses *Confirm email*. */
export default async function ConfirmPage({ params }: { params: Promise<{ token: string }> }) {
  const { token } = await params;
  const link = await inspect(token, "confirm");
  const csrf = await formToken();
  return (
    <Shell>
      <ConfirmViews
        csrf={csrf}
        token={token}
        initial={link.kind === "gone" ? { view: "expired" } : { view: "confirm", email: link.email, answer: 0, failed: false }}
      />
    </Shell>
  );
}
