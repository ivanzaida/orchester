import { redirect } from "next/navigation";

import { message, type Message } from "@/lib/i18n";
import type { ApiFailure, ApiResult, CallOptions } from "@/lib/server/api";
import { failureMessage, leaveIfEnded } from "@/lib/server/failures";
import { endSession, formIsGenuine, signedIn } from "@/lib/server/visitor";

/** What every form of the signed-in pages checks first: a genuine form from someone who is still signed in. */
export async function actor(form: FormData): Promise<CallOptions | Message> {
  if (!(await formIsGenuine(form))) {
    return message("error.formExpired");
  }
  const options = await signedIn();
  if (options === null) {
    // A cookie past its own 12 hours, or one this admin cannot open: nothing to keep.
    await endSession();
    redirect("/admin/signin");
  }
  return options;
}

export function isMessage(value: CallOptions | Message): value is Message {
  return "key" in value;
}

/**
 * The words for a failed call: a session the server ended sends the person to sign in; anything else is the
 * general failure message.
 */
export async function failed(failure: ApiFailure): Promise<Message> {
  await leaveIfEnded(failure);
  return failureMessage(failure, Date.now());
}

/** A call's failure as a message, or `null` when it worked. */
export async function outcome(result: ApiResult<unknown>): Promise<Message | null> {
  return result.ok ? null : failed(result.error);
}
