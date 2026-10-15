import type { Metadata } from "next";

import { t } from "@/lib/i18n";
import { inspect } from "@/lib/server/link";
import { formToken } from "@/lib/server/visitor";
import { Shell } from "@/components/page";

import { ResetViews } from "../views";

export const metadata: Metadata = { title: t("reset.pageTitle") };

/** Where the reset email's link lands: the new password form, or the link-gone view. */
export default async function ResetPage({ params }: { params: Promise<{ token: string }> }) {
  const { token } = await params;
  const link = await inspect(token, "reset");
  const csrf = await formToken();
  return (
    <Shell>
      <ResetViews
        csrf={csrf}
        token={token}
        initial={link.kind === "gone" ? { view: "expired" } : { view: "form", email: link.email, attempt: 0, errors: {} }}
      />
    </Shell>
  );
}
