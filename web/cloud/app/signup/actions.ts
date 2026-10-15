"use server";

import { redirect } from "next/navigation";

import { field } from "@/lib/form";
import { message, type Message } from "@/lib/i18n";
import { signUp } from "@/lib/server/cloud";
import { failureMessage } from "@/lib/server/failures";
import { anonymous, formIsGenuine, setSignupFlow } from "@/lib/server/visitor";
import { normalizeEmail, signUpErrors, type FieldErrors, type SignUpInput } from "@/lib/validation";

export interface SignUpState {
  /** What the visitor typed, kept after a round trip; the password never comes back. */
  readonly email: string;
  readonly displayName: string;
  readonly errors: FieldErrors<keyof SignUpInput>;
  readonly formError?: Message;
}

export async function signUpAction(_: SignUpState, form: FormData): Promise<SignUpState> {
  const input = { email: field(form, "email"), displayName: field(form, "displayName"), password: field(form, "password") };
  const kept = { email: input.email, displayName: input.displayName };
  if (!(await formIsGenuine(form))) {
    return { ...kept, errors: {}, formError: message("error.formExpired") };
  }
  const errors = signUpErrors(input);
  const email = normalizeEmail(input.email);
  if (Object.keys(errors).length > 0 || email === null) {
    return { ...kept, errors };
  }
  const now = Date.now();
  const result = await signUp({ ...input, email, displayName: input.displayName.trim() }, await anonymous());
  if (!result.ok) {
    return { ...kept, errors: {}, formError: failureMessage(result.error, "error.tooManyEmails", now) };
  }
  await setSignupFlow({ email, sentAt: now, resent: false });
  redirect("/signup/sent");
}
