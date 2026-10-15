import type { Metadata } from "next";
import { redirect } from "next/navigation";

import { t } from "@/lib/i18n";
import { me } from "@/lib/server/cloud";
import { CloudUnavailableError } from "@/lib/server/failures";
import { formToken, signedIn } from "@/lib/server/visitor";
import { Address, Card, Head, Text, Top } from "@orchester/ui/components/card";
import { Shell } from "@/components/page";

import { SignOut } from "./sign-out";

export const metadata: Metadata = { title: t("home.pageTitle") };

/**
 * After sign-in. The account page (`/account`) is not built yet, so this says plainly who is signed in, that the
 * account is managed in the app for now, and offers *Sign out*. Without a session it is the sign-in page.
 */
export default async function HomePage() {
  const session = await signedIn();
  if (session === null) {
    redirect("/signin");
  }
  const result = await me(session);
  if (!result.ok) {
    if (result.error.kind === "refused" && result.error.code === "unauthorized") {
      redirect("/signin");
    }
    throw new CloudUnavailableError(result.error);
  }
  const account = result.value;
  return (
    <Shell>
      <Card>
        <Top>
          <Head title={t("home.title")} description={t("home.description", { name: account.displayName })}
            tile={{ icon: "circle-check", tone: "success" }} />
          <Address email={account.email} />
        </Top>
        <Text>{t("home.next")}</Text>
        <SignOut csrf={await formToken()} />
      </Card>
    </Shell>
  );
}
