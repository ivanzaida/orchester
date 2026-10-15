import { redirect } from "next/navigation";

import { me } from "@/lib/server/cloud";
import { endSession, sessionEnded, signedIn } from "@/lib/server/visitor";

/**
 * Where a page sends a visitor whose session the cloud ended: a page cannot change cookies, this route can. It forgets
 * the session only after the cloud itself says it is gone (or the cookie is past its own expiry), so a link to it from
 * another site cannot sign anyone out.
 */
export async function GET(): Promise<never> {
  const session = await signedIn();
  if (session === null) {
    await endSession();
    redirect("/signin");
  }
  const account = await me(session);
  if (!account.ok && sessionEnded(account.error)) {
    await endSession();
    redirect("/signin");
  }
  redirect("/account");
}
