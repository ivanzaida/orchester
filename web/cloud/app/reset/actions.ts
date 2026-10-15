"use server";

import { field } from "@/lib/form";
import { message, type Message } from "@/lib/i18n";
import { finishReset } from "@/lib/server/cloud";
import { failureMessage } from "@/lib/server/failures";
import { anonymous, endSession, formIsGenuine } from "@/lib/server/visitor";
import { newPasswordErrors, type FieldErrors, type NewPasswordInput } from "@/lib/validation";

/** The reset page's views: the form (`hEaQO`, errors `jYzyy`), changed (`KGnzN`), link gone (`w4yqZz`). */
export type ResetState =
  | {
      readonly view: "form";
      readonly email: string;
      readonly attempt: number;
      readonly errors: FieldErrors<keyof NewPasswordInput>;
      readonly error?: Message;
    }
  | { readonly view: "done" }
  | { readonly view: "expired" };

export async function resetAction(previous: ResetState, form: FormData): Promise<ResetState> {
  if (previous.view !== "form") {
    return previous;
  }
  const base = { view: "form", email: previous.email, attempt: previous.attempt + 1, errors: {} } as const;
  if (!(await formIsGenuine(form))) {
    return { ...base, error: message("error.formExpired") };
  }
  const input = { password: field(form, "password"), repeat: field(form, "repeat") };
  const errors = newPasswordErrors(input);
  if (Object.keys(errors).length > 0) {
    return { ...base, errors };
  }
  const now = Date.now();
  const result = await finishReset(field(form, "token"), input.password, await anonymous());
  if (result.ok) {
    // The cloud signed out every device of the account, this browser's session among them.
    await endSession();
    return { view: "done" };
  }
  if (result.error.kind === "refused" && result.error.code === "invalid_link") {
    return { view: "expired" };
  }
  return { ...base, error: failureMessage(result.error, "error.tooManyTries", now) };
}
