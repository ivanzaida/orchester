"use server";

import { confirmFailure } from "@/lib/confirm";
import { field } from "@/lib/form";
import { message, type Message } from "@/lib/i18n";
import { confirmEmail } from "@/lib/server/cloud";
import { anonymous, formIsGenuine } from "@/lib/server/visitor";

/**
 * The confirmation landing's views: the password asked (`Bdcjx`), a wrong answer (`CVRpO`), confirmed (`oFMIA`), and
 * the link gone (`X2Cij0`), which the page knows only when it can inspect the link before anything is typed.
 */
export type ConfirmState =
  | {
      readonly view: "confirm";
      readonly email: string | null;
      /** Counts the answers, so the password field comes back empty after each one. */
      readonly answer: number;
      /** The one answer for any failure (`401 confirm_failed`): it offers *Send a new link*. */
      readonly failed: boolean;
      readonly error?: Message;
    }
  | { readonly view: "confirmed"; readonly email: string | null }
  | { readonly view: "expired" };

/**
 * *Confirm email* with the password chosen at sign-up (decision 65). A wrong password, another sign-up's link and a
 * link that ended meanwhile read alike: the message never says which part was wrong.
 */
export async function confirmAction(previous: ConfirmState, form: FormData): Promise<ConfirmState> {
  if (previous.view !== "confirm") {
    return previous;
  }
  const base = { view: "confirm", email: previous.email, answer: previous.answer + 1 } as const;
  if (!(await formIsGenuine(form))) {
    return { ...base, failed: false, error: message("error.formExpired") };
  }
  const now = Date.now();
  const result = await confirmEmail(field(form, "token"), field(form, "password"), await anonymous());
  if (result.ok) {
    return { view: "confirmed", email: previous.email };
  }
  return { ...base, ...confirmFailure(result.error, now) };
}
