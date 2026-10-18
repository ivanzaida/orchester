"use server";

import { redirect } from "next/navigation";

import { field } from "@/lib/form";
import { message, type Message } from "@/lib/i18n";
import { forgetChallenge, waitingChallenge } from "@/lib/server/challenge";
import { failureMessage, waitMessage } from "@/lib/server/failures";
import { finishSignIn } from "@/lib/server/two-factor";
import { anonymous, formIsGenuine, startSession } from "@/lib/server/visitor";

import { endPreviousSession } from "../actions";

/** How the last code ended; `next` goes on with a whole page load, as the password step does. */
export type CodeOutcome =
  | { readonly kind: "none" }
  | { readonly kind: "wrong" }
  | { readonly kind: "expired" }
  | { readonly kind: "failed"; readonly error: Message }
  | { readonly kind: "next"; readonly href: string };

export interface CodeState {
  /** Counts the answers, so the form knows a new one came and empties the code after it. */
  readonly attempt: number;
  readonly outcome: CodeOutcome;
}

function refusal(failure: Parameters<typeof failureMessage>[0], now: number): CodeOutcome {
  if (failure.kind === "refused" && failure.code === "invalid_code") {
    return { kind: "wrong" };
  }
  if (failure.kind === "refused" && failure.code === "challenge_expired") {
    return { kind: "expired" };
  }
  if (failure.kind === "refused" && failure.code === "rate_limited") {
    return { kind: "failed", error: waitMessage("signin.code.tooMany", failure.retryAfter ?? 60, now) };
  }
  return { kind: "failed", error: failureMessage(failure, "signin.code.tooMany", now) };
}

/**
 * The second step of a sign-in with two-factor authentication: the code from the app, or a recovery code, finishes the
 * challenge the password step kept. A wrong code may be tried again; an ended challenge means signing in again.
 */
export async function codeAction(previous: CodeState, form: FormData): Promise<CodeState> {
  const attempt = previous.attempt + 1;
  if (!(await formIsGenuine(form))) {
    return { attempt, outcome: { kind: "failed", error: message("error.formExpired") } };
  }
  const waiting = await waitingChallenge();
  if (waiting === null) {
    return { attempt, outcome: { kind: "expired" } };
  }
  const result = await finishSignIn(waiting.challenge, field(form, "code"), await anonymous());
  if (!result.ok) {
    const outcome = refusal(result.error, Date.now());
    if (outcome.kind === "expired") {
      await forgetChallenge();
    }
    return { attempt, outcome };
  }
  await forgetChallenge();
  await endPreviousSession();
  await startSession(result.value.token);
  if (waiting.next !== null) {
    return { attempt, outcome: { kind: "next", href: waiting.next } };
  }
  redirect("/account");
}
