"use server";

import { redirect } from "next/navigation";

import { field } from "@/lib/form";
import { message, type Message } from "@/lib/i18n";
import { requestReset } from "@/lib/server/cloud";
import { failureMessage } from "@/lib/server/failures";
import { anonymous, formIsGenuine, setForgotFlow } from "@/lib/server/visitor";
import { normalizeEmail } from "@/lib/validation";

export interface ForgotState {
  readonly email?: string;
  readonly error?: Message;
}

/** *Send reset link*, also *Send a new link* on an expired reset link; the answer is the same for any address. */
export async function forgotAction(_: ForgotState, form: FormData): Promise<ForgotState> {
  const typed = field(form, "email");
  if (!(await formIsGenuine(form))) {
    return { email: typed, error: message("error.formExpired") };
  }
  const email = normalizeEmail(typed);
  if (email === null) {
    return { email: typed, error: message("error.emailInvalid") };
  }
  const now = Date.now();
  const result = await requestReset(email, await anonymous());
  if (!result.ok) {
    return { email: typed, error: failureMessage(result.error, "error.tooManyEmails", now) };
  }
  await setForgotFlow({ email });
  redirect("/forgot/sent");
}
