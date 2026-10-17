"use server";

import { revalidatePath } from "next/cache";
import { redirect } from "next/navigation";

import { field } from "@/lib/form";
import { message, type Message } from "@/lib/i18n";
import { changePassword, declineInvite, deleteAccount, rename, revokeDevice } from "@/lib/server/account";
import { signOut } from "@/lib/server/cloud";
import { failureMessage } from "@/lib/server/failures";
import { formSession, isMessage } from "@/lib/server/form-session";
import { endSession, leaveIfEnded, sessionEnded } from "@/lib/server/visitor";
import { collect, displayNameError, newPasswordErrors, type FieldErrors } from "@/lib/validation";

export interface FormState {
  readonly error?: Message;
}

/**
 * Ends this browser's session at the cloud, then forgets it here. When the cloud cannot be told, the session stays
 * and the visitor reads why: a cookie dropped here while its token still works at the cloud would only look signed out.
 */
export async function signOutAction(_: FormState, form: FormData): Promise<FormState> {
  const options = await formSession(form);
  if (isMessage(options)) {
    return { error: options };
  }
  const result = await signOut(options);
  const ended = result.ok || sessionEnded(result.error);
  if (!ended) {
    return { error: failureMessage(result.error, "error.tooManyTries", Date.now()) };
  }
  await endSession();
  redirect("/signin");
}

export async function renameAction(_: FormState, form: FormData): Promise<FormState> {
  const options = await formSession(form);
  if (isMessage(options)) {
    return { error: options };
  }
  const name = field(form, "displayName");
  const invalid = displayNameError(name);
  if (invalid !== undefined) {
    return { error: invalid };
  }
  const result = await rename(name.trim(), options);
  if (!result.ok) {
    await leaveIfEnded(result.error);
    return { error: failureMessage(result.error, "error.tooManyTries", Date.now()) };
  }
  revalidatePath("/account");
  return {};
}

export type PasswordState =
  | { readonly view: "closed"; readonly changed: boolean }
  | {
      readonly view: "open";
      /** Counts the answers, so the form empties its fields after each one (`ZyAsL`). */
      readonly answer: number;
      readonly errors: FieldErrors<"current" | "password" | "repeat">;
      readonly error?: Message;
    };

function passwordErrors(form: FormData): FieldErrors<"current" | "password" | "repeat"> {
  const current = field(form, "current") === "" ? message("account.settings.currentEmpty") : undefined;
  const next = newPasswordErrors({ password: field(form, "password"), repeat: field(form, "repeat") });
  return collect({ current, password: next.password, repeat: next.repeat });
}

export async function passwordAction(previous: PasswordState, form: FormData): Promise<PasswordState> {
  const answer = previous.view === "open" ? previous.answer + 1 : 1;
  const options = await formSession(form);
  if (isMessage(options)) {
    return { view: "open", answer, errors: {}, error: options };
  }
  const errors = passwordErrors(form);
  if (Object.keys(errors).length > 0) {
    return { view: "open", answer, errors };
  }
  const result = await changePassword(field(form, "current"), field(form, "password"), options);
  if (!result.ok) {
    await leaveIfEnded(result.error);
  }
  if (result.ok) {
    // Other devices were signed out; the page reads the list again.
    revalidatePath("/account");
    return { view: "closed", changed: true };
  }
  if (result.error.kind === "refused" && result.error.code === "invalid_credentials") {
    return { view: "open", answer, errors: { current: message("account.settings.currentWrong") } };
  }
  return { view: "open", answer, errors: {}, error: failureMessage(result.error, "error.tooManyTries", Date.now()) };
}

export type DeleteState =
  | { readonly view: "closed" }
  | { readonly view: "open"; readonly errors: FieldErrors<"password">; readonly error?: Message };

/**
 * Deletes the account with its password (`DELETE /me`). A wrong password is said at its field; a deletion ends this
 * browser's session, as every other device's, and leaves for sign-in.
 */
export async function deleteAccountAction(_: DeleteState, form: FormData): Promise<DeleteState> {
  const options = await formSession(form);
  if (isMessage(options)) {
    return { view: "open", errors: {}, error: options };
  }
  const password = field(form, "password");
  if (password === "") {
    return { view: "open", errors: { password: message("account.settings.deleteEmpty") } };
  }
  const result = await deleteAccount(password, options);
  if (result.ok) {
    await endSession();
    redirect("/signin");
  }
  await leaveIfEnded(result.error);
  if (result.error.kind === "refused" && result.error.code === "invalid_credentials") {
    return { view: "open", errors: { password: message("account.settings.deleteWrong") } };
  }
  return { view: "open", errors: {}, error: failureMessage(result.error, "error.tooManyTries", Date.now()) };
}

export interface DeviceState {
  readonly signedOut?: string;
  readonly error?: Message;
}

/** Signs another device out at once, without a question first (the designer's notes); this one signs out as a whole. */
export async function signOutDeviceAction(_: DeviceState, form: FormData): Promise<DeviceState> {
  const options = await formSession(form);
  if (isMessage(options)) {
    return { error: options };
  }
  const id = field(form, "device");
  const result = await revokeDevice(id, options);
  if (!result.ok) {
    await leaveIfEnded(result.error);
  }
  // `404`: the device was already signed out, which is what was asked for.
  const gone = result.ok || (result.error.kind === "refused" && result.error.code === "not_found");
  return gone ? { signedOut: id } : { error: failureMessage(result.error, "error.tooManyTries", Date.now()) };
}

export async function declineAction(_: FormState, form: FormData): Promise<FormState> {
  const options = await formSession(form);
  if (isMessage(options)) {
    return { error: options };
  }
  const result = await declineInvite(field(form, "invite"), options);
  if (!result.ok) {
    await leaveIfEnded(result.error);
  }
  // `404`: the invite already ended (declined elsewhere, accepted or expired), which is what was asked for.
  const gone = result.ok || (result.error.kind === "refused" && result.error.code === "not_found");
  if (gone) {
    revalidatePath("/account");
    return {};
  }
  return { error: failureMessage(result.error, "error.tooManyTries", Date.now()) };
}
