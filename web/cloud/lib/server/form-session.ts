import { redirect } from "next/navigation";

import { message, type Message } from "@/lib/i18n";
import type { CallOptions } from "@/lib/server/api";
import { endSession, formIsGenuine, signedIn } from "@/lib/server/visitor";

/**
 * The checks every form of the Account page makes first: a genuine form from a visitor who is still signed in. Kept
 * out of the actions' modules: everything a `"use server"` module exports is an action a browser can call, and this
 * answers the session's token.
 */
export async function formSession(form: FormData): Promise<CallOptions | Message> {
  if (!(await formIsGenuine(form))) {
    return message("error.formExpired");
  }
  const options = await signedIn();
  if (options === null) {
    // A cookie past its own 12 hours, or one this site cannot open: nothing to keep.
    await endSession();
    redirect("/signin");
  }
  return options;
}

export function isMessage(value: CallOptions | Message): value is Message {
  return "key" in value;
}
