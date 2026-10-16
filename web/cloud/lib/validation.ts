import { message, type Message } from "@/lib/i18n";

/**
 * The cloud's input rules (`crates/cloud` `accounts/input.rs` and `passwords.rs`), checked in the browser before
 * sending and again on the web server; the cloud checks once more.
 */
export const PASSWORD_MIN = 12;
export const PASSWORD_MAX = 256;
export const DISPLAY_NAME_MAX = 100;
const EMAIL_MAX = 254;
const LOCAL_MAX = 64;
const FORBIDDEN = /[\s\p{Cc}@<>,;:"()[\]\\]/u;

function length(value: string): number {
  // Unicode scalar values, as the cloud counts (`chars().count()`).
  return Array.from(value).length;
}

// The cloud measures an email in UTF-8 bytes.
function bytes(value: string): number {
  return new TextEncoder().encode(value).length;
}

function plainPart(part: string): boolean {
  return part.length > 0 && !FORBIDDEN.test(part);
}

/** The email as the cloud keeps it (trimmed, lowercased), or `null` when the cloud would refuse it. */
export function normalizeEmail(value: string): string | null {
  const email = value.trim().toLowerCase();
  const at = email.indexOf("@");
  if (at < 0) {
    return null;
  }
  const local = email.slice(0, at);
  const domain = email.slice(at + 1);
  const dotted = domain.includes(".") && !domain.startsWith(".") && !domain.endsWith(".") && !domain.includes("..");
  const fits = bytes(email) <= EMAIL_MAX && bytes(local) <= LOCAL_MAX;
  return fits && plainPart(local) && plainPart(domain) && dotted ? email : null;
}

export function emailError(value: string): Message | undefined {
  return normalizeEmail(value) === null ? message("error.emailInvalid") : undefined;
}

export function passwordError(value: string): Message | undefined {
  const count = length(value);
  if (count < PASSWORD_MIN) {
    return message("error.passwordShort", { count });
  }
  return count > PASSWORD_MAX ? message("error.passwordLong", { count }) : undefined;
}

export function displayNameError(value: string): Message | undefined {
  const name = value.trim();
  const count = length(name);
  if (count === 0) {
    return message("error.displayNameEmpty");
  }
  return count > DISPLAY_NAME_MAX ? message("error.displayNameLong", { count }) : undefined;
}

export function repeatError(password: string, repeat: string): Message | undefined {
  return password === repeat ? undefined : message("error.passwordsDiffer");
}

export type FieldErrors<F extends string> = Partial<Record<F, Message>>;

/** Drops the fields without an error, so an empty object means the input is valid. */
export function collect<F extends string>(errors: Record<F, Message | undefined>): FieldErrors<F> {
  const found: FieldErrors<F> = {};
  for (const [field, error] of Object.entries(errors) as [F, Message | undefined][]) {
    if (error !== undefined) {
      found[field] = error;
    }
  }
  return found;
}

export interface SignUpInput {
  email: string;
  displayName: string;
  password: string;
}

export function signUpErrors(input: SignUpInput): FieldErrors<keyof SignUpInput> {
  return collect({
    email: emailError(input.email),
    displayName: displayNameError(input.displayName),
    password: passwordError(input.password),
  });
}

export interface NewPasswordInput {
  password: string;
  repeat: string;
}

export function newPasswordErrors(input: NewPasswordInput): FieldErrors<keyof NewPasswordInput> {
  const password = passwordError(input.password);
  return collect({ password, repeat: repeatError(input.password, input.repeat) });
}
