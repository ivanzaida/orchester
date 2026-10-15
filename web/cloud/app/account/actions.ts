"use server";

import { revalidatePath } from "next/cache";
import { redirect } from "next/navigation";

import { field } from "@/lib/form";
import { message, type Message } from "@/lib/i18n";
import type { CallOptions } from "@/lib/server/api";
import { changePassword, declineInvite, rename, revokeDevice } from "@/lib/server/account";
import { signOut } from "@/lib/server/cloud";
import { failureMessage } from "@/lib/server/failures";
import { endSession, formIsGenuine, signedIn } from "@/lib/server/visitor";
import { collect, displayNameError, newPasswordErrors, type FieldErrors } from "@/lib/validation";

export interface FormState {
  readonly error?: Message;
}

/** The checks every form of the page makes first: a genuine form from a visitor who is still signed in. */
async function session(form: FormData): Promise<CallOptions | Message> {
  if (!(await formIsGenuine(form))) {
    return message("error.formExpired");
  }
  const options = await signedIn();
  if (options === null) {
    redirect("/signin");
  }
  return options;
}

function isMessage(value: CallOptions | Message): value is Message {
  return "key" in value;
}

/**
 * Ends this browser's session at the cloud, then forgets it here. When the cloud cannot be told, the session stays
 * and the visitor reads why: a cookie dropped here while its token still works at the cloud would only look signed out.
 */
export async function signOutAction(_: FormState, form: FormData): Promise<FormState> {
  const options = await session(form);
  if (isMessage(options)) {
    return { error: options };
  }
  const result = await signOut(options);
  const ended = result.ok || (result.error.kind === "refused" && result.error.code === "unauthorized");
  if (!ended) {
    return { error: failureMessage(result.error, "error.tooManyTries", Date.now()) };
  }
  await endSession();
  redirect("/signin");
}

export async function renameAction(_: FormState, form: FormData): Promise<FormState> {
  const options = await session(form);
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
  const options = await session(form);
  if (isMessage(options)) {
    return { view: "open", answer, errors: {}, error: options };
  }
  const errors = passwordErrors(form);
  if (Object.keys(errors).length > 0) {
    return { view: "open", answer, errors };
  }
  const result = await changePassword(field(form, "current"), field(form, "password"), options);
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

export interface DeviceState {
  readonly signedOut?: string;
  readonly error?: Message;
}

/** Signs another device out at once, without a question first (the designer's notes); this one signs out as a whole. */
export async function signOutDeviceAction(_: DeviceState, form: FormData): Promise<DeviceState> {
  const options = await session(form);
  if (isMessage(options)) {
    return { error: options };
  }
  const id = field(form, "device");
  const result = await revokeDevice(id, options);
  // `404`: the device was already signed out, which is what was asked for.
  const gone = result.ok || (result.error.kind === "refused" && result.error.code === "not_found");
  return gone ? { signedOut: id } : { error: failureMessage(result.error, "error.tooManyTries", Date.now()) };
}

export async function declineAction(_: FormState, form: FormData): Promise<FormState> {
  const options = await session(form);
  if (isMessage(options)) {
    return { error: options };
  }
  const result = await declineInvite(field(form, "invite"), options);
  if (result.ok) {
    revalidatePath("/account");
    return {};
  }
  if (result.error.kind === "refused" && result.error.code === "not_found") {
    return { error: message("account.invites.declineNotYet") };
  }
  return { error: failureMessage(result.error, "error.tooManyTries", Date.now()) };
}
