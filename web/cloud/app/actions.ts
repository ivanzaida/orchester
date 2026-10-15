"use server";

import { redirect } from "next/navigation";

import { message, type Message } from "@/lib/i18n";
import { signOut } from "@/lib/server/cloud";
import { failureMessage } from "@/lib/server/failures";
import { endSession, formIsGenuine, signedIn } from "@/lib/server/visitor";

export interface SignOutState {
  readonly error?: Message;
}

/**
 * Ends the web session at the cloud, then forgets it here. When the cloud cannot be told, the session stays and the
 * visitor reads why: a cookie dropped here while its token still works at the cloud would only look signed out.
 */
export async function signOutAction(_: SignOutState, form: FormData): Promise<SignOutState> {
  if (!(await formIsGenuine(form))) {
    return { error: message("error.formExpired") };
  }
  const session = await signedIn();
  if (session !== null) {
    const result = await signOut(session);
    const ended = result.ok || (result.error.kind === "refused" && result.error.code === "unauthorized");
    if (!ended) {
      return { error: failureMessage(result.error, "error.tooManyTries", Date.now()) };
    }
  }
  await endSession();
  redirect("/signin");
}
