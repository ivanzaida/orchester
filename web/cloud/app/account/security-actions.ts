"use server";

import { revalidatePath } from "next/cache";

import { field } from "@/lib/form";
import { message, type Message } from "@/lib/i18n";
import type { ApiFailure } from "@/lib/server/api";
import { failureMessage } from "@/lib/server/failures";
import { formSession, isMessage } from "@/lib/server/form-session";
import { confirmSetup, newRecoveryCodes, qrCode, startSetup, stepUp, turnOff } from "@/lib/server/two-factor";
import { leaveIfEnded } from "@/lib/server/visitor";
import type { QrCode } from "@/lib/two-factor";
import type { FieldErrors } from "@/lib/validation";

/**
 * The Security section's forms (`docs/cloud-api.md`, "Two-factor authentication"). Each answer counts in `answer`, so
 * its form empties its fields; a refusal is said at its field, anything else under the form.
 */

function refused(failure: ApiFailure, code: string): boolean {
  return failure.kind === "refused" && failure.code === code;
}

async function failed(failure: ApiFailure): Promise<Message> {
  await leaveIfEnded(failure);
  return failureMessage(failure, "error.tooManyTries", Date.now());
}

export type SetupState =
  | { readonly view: "closed" }
  | { readonly view: "password"; readonly answer: number; readonly error?: Message }
  | {
      readonly view: "scan";
      readonly answer: number;
      readonly secret: string;
      readonly qr: QrCode;
      readonly error?: Message;
    }
  | { readonly view: "codes"; readonly codes: readonly string[] };

function answerOf(previous: SetupState): number {
  return previous.view === "password" || previous.view === "scan" ? previous.answer + 1 : 1;
}

async function start(form: FormData, answer: number): Promise<SetupState> {
  const options = await formSession(form);
  if (isMessage(options)) {
    return { view: "password", answer, error: options };
  }
  const password = field(form, "password");
  if (password === "") {
    return { view: "password", answer, error: message("account.security.passwordEmpty") };
  }
  const result = await startSetup(password, options);
  if (result.ok) {
    return { view: "scan", answer, secret: result.value.secret, qr: qrCode(result.value.uri) };
  }
  if (refused(result.error, "conflict")) {
    // Turned on meanwhile, from another device: the page shows it on.
    revalidatePath("/account");
    return { view: "closed" };
  }
  const error = refused(result.error, "invalid_credentials")
    ? message("account.security.passwordWrong")
    : await failed(result.error);
  return { view: "password", answer, error };
}

async function confirm(form: FormData, scan: Extract<SetupState, { view: "scan" }>): Promise<SetupState> {
  const answer = scan.answer + 1;
  const options = await formSession(form);
  if (isMessage(options)) {
    return { ...scan, answer, error: options };
  }
  const code = field(form, "code");
  if (code.trim() === "") {
    return { ...scan, answer, error: message("account.security.codeEmpty") };
  }
  const result = await confirmSetup(code, options);
  if (result.ok) {
    revalidatePath("/account");
    return { view: "codes", codes: result.value };
  }
  if (refused(result.error, "not_found")) {
    return { view: "password", answer, error: message("account.security.setupEnded") };
  }
  const error = refused(result.error, "invalid_code") ? message("account.security.codeWrong") : await failed(result.error);
  return { ...scan, answer, error };
}

/** *Turn on*: the password, then a code from the app that scanned the QR code, then the recovery codes, shown once. */
export async function turnOnAction(previous: SetupState, form: FormData): Promise<SetupState> {
  if (field(form, "step") === "code" && previous.view === "scan") {
    return confirm(form, previous);
  }
  return start(form, answerOf(previous));
}

export type OffState =
  | { readonly view: "closed" }
  | { readonly view: "open"; readonly answer: number; readonly errors: FieldErrors<"password" | "code">; readonly error?: Message };

/** *Turn off*: the password and a code from the app or a recovery code. */
export async function turnOffAction(previous: OffState, form: FormData): Promise<OffState> {
  const answer = previous.view === "open" ? previous.answer + 1 : 1;
  const options = await formSession(form);
  if (isMessage(options)) {
    return { view: "open", answer, errors: {}, error: options };
  }
  const password = field(form, "password");
  const code = field(form, "code");
  if (password === "" || code.trim() === "") {
    const errors = {
      ...(password === "" ? { password: message("account.security.passwordEmpty") } : {}),
      ...(code.trim() === "" ? { code: message("account.security.anyCodeEmpty") } : {}),
    };
    return { view: "open", answer, errors };
  }
  const result = await turnOff(password, code, options);
  if (result.ok || refused(result.error, "not_found")) {
    revalidatePath("/account");
    return { view: "closed" };
  }
  if (refused(result.error, "invalid_credentials")) {
    return { view: "open", answer, errors: { password: message("account.security.passwordWrong") } };
  }
  if (refused(result.error, "invalid_code")) {
    return { view: "open", answer, errors: { code: message("account.security.anyCodeWrong") } };
  }
  return { view: "open", answer, errors: {}, error: await failed(result.error) };
}

export type CodeFormState =
  | { readonly view: "closed" }
  | { readonly view: "open"; readonly answer: number; readonly error?: Message }
  | { readonly view: "codes"; readonly codes: readonly string[] };

/** The one code a form takes, or what to say instead of sending it. */
async function oneCode(form: FormData) {
  const options = await formSession(form);
  if (isMessage(options)) {
    return { error: options };
  }
  const code = field(form, "code");
  return code.trim() === "" ? { error: message("account.security.anyCodeEmpty") } : { options, code };
}

/** *Make new codes*: a code replaces every recovery code with ten new ones, shown once. */
export async function recoveryCodesAction(previous: CodeFormState, form: FormData): Promise<CodeFormState> {
  const answer = previous.view === "open" ? previous.answer + 1 : 1;
  const checked = await oneCode(form);
  if ("error" in checked) {
    return { view: "open", answer, error: checked.error };
  }
  const result = await newRecoveryCodes(checked.code, checked.options);
  if (result.ok) {
    revalidatePath("/account");
    return { view: "codes", codes: result.value };
  }
  const error = refused(result.error, "invalid_code") ? message("account.security.anyCodeWrong") : await failed(result.error);
  return { view: "open", answer, error };
}

/** *Verify* this browser: a code makes its session a two-factor one, which may accept invites. */
export async function stepUpAction(previous: CodeFormState, form: FormData): Promise<CodeFormState> {
  const answer = previous.view === "open" ? previous.answer + 1 : 1;
  const checked = await oneCode(form);
  if ("error" in checked) {
    return { view: "open", answer, error: checked.error };
  }
  const result = await stepUp(checked.code, checked.options);
  if (result.ok) {
    revalidatePath("/account");
    return { view: "closed" };
  }
  const error = refused(result.error, "invalid_code") ? message("account.security.anyCodeWrong") : await failed(result.error);
  return { view: "open", answer, error };
}
