"use server";

import { headers } from "next/headers";
import { redirect } from "next/navigation";

import { nextAfterSignIn } from "@/lib/authorize";
import { field } from "@/lib/form";
import { message, type Message } from "@/lib/i18n";
import type { ApiFailure } from "@/lib/server/api";
import { signIn, signOut } from "@/lib/server/cloud";
import { webDevice } from "@/lib/server/device";
import { failureMessage, waitMessage } from "@/lib/server/failures";
import { anonymous, formIsGenuine, signedIn, startSession } from "@/lib/server/visitor";

/** How the last sign-in ended; `limited` carries both texts and how long the wait is from now. */
export type SignInOutcome =
  | { readonly kind: "none" }
  | { readonly kind: "wrong" }
  | { readonly kind: "unconfirmed"; readonly email: string }
  | { readonly kind: "limited"; readonly error: Message; readonly reason: Message; readonly ms: number }
  | { readonly kind: "failed"; readonly error: Message }
  /** Signed in, and back to `/authorize` with a whole page load: its own CSP lets it post to the server admin. */
  | { readonly kind: "next"; readonly href: string };

export interface SignInState {
  readonly email: string;
  /** Counts the answers, so the form knows a new one came (and clears the password after a wrong one). */
  readonly attempt: number;
  readonly outcome: SignInOutcome;
}

function refusal(failure: ApiFailure, email: string, now: number): SignInOutcome {
  if (failure.kind === "refused" && failure.code === "invalid_credentials") {
    return { kind: "wrong" };
  }
  if (failure.kind === "refused" && failure.code === "email_unconfirmed") {
    return { kind: "unconfirmed", email };
  }
  if (failure.kind === "refused" && failure.code === "rate_limited") {
    const seconds = failure.retryAfter ?? 60;
    const until = now + seconds * 1000;
    const reason = message("signin.unavailableUntil", {}, until);
    return { kind: "limited", error: waitMessage("signin.tooMany", seconds, now), reason, ms: seconds * 1000 };
  }
  return { kind: "failed", error: failureMessage(failure, "signin.tooMany", now) };
}

/**
 * Signing in again replaces this browser's session; the old one is ended at the cloud first, so its token does not
 * stay valid unseen until its 12 hours are over. If the cloud cannot be told, the new sign-in still goes ahead: the
 * visitor asked for it, and the old token ends on its own within 12 hours.
 */
async function endPreviousSession(): Promise<void> {
  const previous = await signedIn();
  if (previous !== null) {
    // Losing this answer is the trade-off above; the new session does not depend on it.
    await signOut(previous);
  }
}

/** Signs in as a `web` device; the session token goes only into the sealed session cookie. */
export async function signInAction(previous: SignInState, form: FormData): Promise<SignInState> {
  const email = field(form, "email").trim();
  const attempt = previous.attempt + 1;
  if (!(await formIsGenuine(form))) {
    return { email, attempt, outcome: { kind: "failed", error: message("error.formExpired") } };
  }
  const now = Date.now();
  const device = webDevice((await headers()).get("user-agent"));
  const result = await signIn({ email, password: field(form, "password"), device }, await anonymous());
  if (!result.ok) {
    return { email, attempt, outcome: refusal(result.error, email, now) };
  }
  await endPreviousSession();
  await startSession(result.value.token);
  const next = nextAfterSignIn(field(form, "next"));
  if (next !== null) {
    return { email, attempt, outcome: { kind: "next", href: next } };
  }
  redirect("/account");
}
