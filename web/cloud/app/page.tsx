import { redirect } from "next/navigation";

import { sessionToken } from "@/lib/server/visitor";

/** The site's root: the account for a signed-in visitor, sign-in for anyone else. */
export default async function Root() {
  redirect((await sessionToken()) === null ? "/signin" : "/account");
}
