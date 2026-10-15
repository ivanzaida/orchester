"use server";

import { redirect } from "next/navigation";

import { field } from "@/lib/form";
import { message, type Message } from "@/lib/i18n";
import { RESEND_COOLDOWN_MS } from "@/lib/resend";
import { resendConfirmation } from "@/lib/server/cloud";
import { failureMessage } from "@/lib/server/failures";
import { anonymous, formIsGenuine, setSignupFlow, signupFlow } from "@/lib/server/visitor";
import { normalizeEmail } from "@/lib/validation";

export interface ResendState {
  readonly error?: Message;
}

/**
 * Sends a new confirmation link and shows *Check your email* for it. A `rate_limited` answer is remembered with its
 * time, so the page says until when sending is unavailable.
 */
async function resend(email: string, resent: boolean): Promise<ResendState> {
  const now = Date.now();
  const result = await resendConfirmation(email, await anonymous());
  if (!result.ok) {
    const failure = result.error;
    if (failure.kind !== "refused" || failure.code !== "rate_limited") {
      return { error: failureMessage(failure, "error.tooManyEmails", now) };
    }
    const previous = await signupFlow();
    const sentAt = previous?.email === email ? previous.sentAt : now;
    await setSignupFlow({ email, sentAt, resent, limitedUntil: now + (failure.retryAfter ?? 60) * 1000 });
    redirect("/signup/sent");
  }
  await setSignupFlow({ email, sentAt: now, resent });
  redirect("/signup/sent");
}

/** *Resend email* on *Check your email*: the address comes from the page's own cookie. */
export async function resendAction(_: ResendState, form: FormData): Promise<ResendState> {
  if (!(await formIsGenuine(form))) {
    return { error: message("error.formExpired") };
  }
  const flow = await signupFlow();
  if (flow === null) {
    redirect("/signup");
  }
  if (Date.now() < flow.sentAt + RESEND_COOLDOWN_MS) {
    // The button is unavailable during the cooldown; a post that arrives anyway sends nothing.
    redirect("/signup/sent");
  }
  return resend(flow.email, true);
}

/** *Resend confirmation* (sign-in) and *Send a new link* (an expired confirmation link): the address is typed. */
export async function resendForAction(_: ResendState, form: FormData): Promise<ResendState> {
  if (!(await formIsGenuine(form))) {
    return { error: message("error.formExpired") };
  }
  const email = normalizeEmail(field(form, "email"));
  if (email === null) {
    return { error: message("error.emailInvalid") };
  }
  return resend(email, true);
}
