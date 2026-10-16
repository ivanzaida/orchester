import { redirect } from "next/navigation";

import { sessionEnded } from "@/lib/server/failures";
import { me } from "@/lib/server/team/auth";
import { endSession, signedIn } from "@/lib/server/visitor";

/**
 * Where a page sends someone whose session the server ended: a page cannot change cookies, this route can. It forgets
 * the session only after the server itself says it is gone (or the cookie is past its own expiry), so a link to it
 * from another site cannot sign anyone out.
 */
export async function GET(): Promise<never> {
  const options = await signedIn();
  if (options === null) {
    await endSession();
    redirect("/admin/signin");
  }
  const user = await me(options);
  if (!user.ok && sessionEnded(user.error)) {
    await endSession();
    redirect("/admin/signin");
  }
  redirect("/admin");
}
