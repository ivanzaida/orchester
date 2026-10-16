"use server";

import { redirect } from "next/navigation";

import { field } from "@/lib/form";
import { message, type Message } from "@/lib/i18n";
import { failureMessage } from "@/lib/server/failures";
import { checkSetupCode } from "@/lib/server/team/auth";
import { anonymous, formIsGenuine, keepSetupCode } from "@/lib/server/visitor";
import { codeError } from "@/lib/validation";

export type SetupOutcome =
  | { readonly kind: "none" }
  | { readonly kind: "wrong"; readonly error: Message }
  | { readonly kind: "limited"; readonly until: number; readonly ms: number }
  | { readonly kind: "failed"; readonly error: Message };

export interface SetupState {
  readonly attempt: number;
  readonly code: string;
  readonly outcome: SetupOutcome;
}

/**
 * Setup step 1: is this the open setup code? Nothing is used up yet; the code is kept sealed in a cookie for step 2,
 * never in the page, a URL or a log.
 */
export async function setupCodeAction(previous: SetupState, form: FormData): Promise<SetupState> {
  const code = field(form, "code").trim();
  const attempt = previous.attempt + 1;
  if (!(await formIsGenuine(form))) {
    return { attempt, code, outcome: { kind: "failed", error: message("error.formExpired") } };
  }
  const invalid = codeError(code);
  if (invalid !== undefined) {
    return { attempt, code, outcome: { kind: "wrong", error: invalid } };
  }
  const result = await checkSetupCode(code, await anonymous());
  if (result.ok) {
    await keepSetupCode(code);
    redirect("/admin/setup/account");
  }
  const failure = result.error;
  if (failure.kind === "refused" && failure.code === "invalid_code") {
    return { attempt, code, outcome: { kind: "wrong", error: message("setup.wrong") } };
  }
  if (failure.kind === "refused" && failure.code === "rate_limited") {
    const seconds = failure.retryAfter ?? 900;
    return { attempt, code: "", outcome: { kind: "limited", until: Date.now() + seconds * 1000, ms: seconds * 1000 } };
  }
  return { attempt, code, outcome: { kind: "failed", error: failureMessage(failure, Date.now()) } };
}
