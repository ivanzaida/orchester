import type { Metadata } from "next";

import { t } from "@/lib/i18n";
import { linkEmail } from "@/lib/server/link";
import { formToken } from "@/lib/server/visitor";
import { Shell } from "@/components/page";

import { ResetViews } from "../views";

export const metadata: Metadata = { title: t("reset.pageTitle") };

/** Where the reset email's link lands: the new password form, or the link-gone view. */
export default async function ResetPage({ params }: { params: Promise<{ token: string }> }) {
  const { token } = await params;
  const email = await linkEmail(token, "reset");
  const csrf = await formToken();
  return (
    <Shell>
      <ResetViews
        csrf={csrf}
        token={token}
        initial={email === null ? { view: "expired" } : { view: "form", email, attempt: 0, errors: {} }}
      />
    </Shell>
  );
}
