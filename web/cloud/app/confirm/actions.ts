"use server";

import { field } from "@/lib/form";
import { message, type Message } from "@/lib/i18n";
import { confirmEmail } from "@/lib/server/cloud";
import { failureMessage } from "@/lib/server/failures";
import { anonymous, formIsGenuine } from "@/lib/server/visitor";

/** The confirmation landing's views: press to confirm (`Bdcjx`), confirmed (`oFMIA`), link gone (`X2Cij0`). */
export type ConfirmState =
  | { readonly view: "confirm"; readonly email: string | null; readonly error?: Message }
  | { readonly view: "confirmed"; readonly email: string | null }
  | { readonly view: "expired" };

/** *Confirm email*: confirming takes this press, never the page's load (mail scanners open every link). */
export async function confirmAction(previous: ConfirmState, form: FormData): Promise<ConfirmState> {
  const email = previous.view === "expired" ? null : previous.email;
  if (!(await formIsGenuine(form))) {
    return { view: "confirm", email, error: message("error.formExpired") };
  }
  const now = Date.now();
  const result = await confirmEmail(field(form, "token"), await anonymous());
  if (result.ok) {
    return { view: "confirmed", email };
  }
  if (result.error.kind === "refused" && result.error.code === "invalid_link") {
    return { view: "expired" };
  }
  return { view: "confirm", email, error: failureMessage(result.error, "error.tooManyTries", now) };
}
