"use server";

import { headers } from "next/headers";
import { redirect } from "next/navigation";

import { field } from "@/lib/form";
import { message, type Message } from "@/lib/i18n";
import { failureMessage } from "@/lib/server/failures";
import { browserDevice } from "@/lib/server/device";
import { signInWithCode } from "@/lib/server/team/auth";
import { anonymous, formIsGenuine, setNothingFlow, startSession } from "@/lib/server/visitor";
import { codeError } from "@/lib/validation";

/** How the last try ended. `limited` carries the time the server allows the next one, for *Try again at 14:46*. */
export type CodeOutcome =
  | { readonly kind: "none" }
  | { readonly kind: "wrong"; readonly error: Message }
  | { readonly kind: "limited"; readonly until: number; readonly ms: number }
  | { readonly kind: "failed"; readonly error: Message };

export interface CodeState {
  /** Counts the answers, so the form knows a new one came. */
  readonly attempt: number;
  readonly code: string;
  readonly outcome: CodeOutcome;
}

/**
 * Signs in with a pairing code or an account code, as an `admin_web` device. The server gives one answer for a wrong,
 * used or expired code, so the page says it once, without telling which.
 */
export async function signInCodeAction(previous: CodeState, form: FormData): Promise<CodeState> {
  const code = field(form, "code").trim();
  const attempt = previous.attempt + 1;
  if (!(await formIsGenuine(form))) {
    return { attempt, code, outcome: { kind: "failed", error: message("error.formExpired") } };
  }
  const invalid = codeError(code);
  if (invalid !== undefined) {
    return { attempt, code, outcome: { kind: "wrong", error: invalid } };
  }
  const device = browserDevice((await headers()).get("user-agent"));
  const result = await signInWithCode({ code, device }, await anonymous());
  if (result.ok) {
    await startSession({ token: result.value.token, via: "code" });
    redirect("/admin");
  }
  const failure = result.error;
  if (failure.kind === "refused" && failure.code === "nothing_to_manage") {
    await setNothingFlow({ name: null, email: null });
    redirect("/admin/nothing");
  }
  if (failure.kind === "refused" && failure.code === "invalid_code") {
    return { attempt, code, outcome: { kind: "wrong", error: message("code.wrong") } };
  }
  if (failure.kind === "refused" && failure.code === "rate_limited") {
    const seconds = failure.retryAfter ?? 900;
    return { attempt, code: "", outcome: { kind: "limited", until: Date.now() + seconds * 1000, ms: seconds * 1000 } };
  }
  return { attempt, code, outcome: { kind: "failed", error: failureMessage(failure, Date.now()) } };
}
