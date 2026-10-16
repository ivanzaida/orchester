"use server";

import { revalidatePath } from "next/cache";

import { field } from "@/lib/form";
import { message, type Message } from "@/lib/i18n";
import type { Role } from "@/lib/model";
import { actor, failed, isMessage } from "@/lib/server/act";
import type { ApiFailure, CallOptions } from "@/lib/server/api";
import { deliverInvite, deliverRevocation } from "@/lib/server/cloud";
import { waitMessage } from "@/lib/server/failures";
import { createInvite, revokeInvite, type NewInvite } from "@/lib/server/team/invites";
import { emailError, normalizeEmail } from "@/lib/validation";

/** Invites from the admin's pages: a project's Invites and People (contract, "Server API" and "Delivering"). */

/** Who an invite is for, from the form: a project with a role, or the server (as an admin or not), or an account. */
type Scope =
  | { readonly kind: "project"; readonly project: string; readonly role: Role }
  | { readonly kind: "server"; readonly admin: boolean }
  | { readonly kind: "account"; readonly user: string };

function scope(form: FormData): Scope {
  const kind = field(form, "kind");
  if (kind === "account") {
    return { kind: "account", user: field(form, "user") };
  }
  if (kind === "project") {
    const role: Role = field(form, "role") === "owner" ? "owner" : "member";
    return { kind: "project", project: field(form, "project"), role };
  }
  return { kind: "server", admin: field(form, "admin") === "yes" };
}

function refusal(failure: ApiFailure): Message | null {
  if (failure.kind !== "refused") {
    return null;
  }
  switch (failure.code) {
    case "rate_limited":
      return waitMessage("invites.limitText", failure.retryAfter ?? 3600, Date.now());
    case "too_many_codes":
      return message("invites.tooManyCodes");
    case "cloud_off":
      return message("invites.unlinkedText");
    default:
      return null;
  }
}

export type EmailState =
  | { readonly view: "form"; readonly answer: number; readonly email: string; readonly error?: Message; readonly limited?: Message }
  | { readonly view: "sent"; readonly answer: number; readonly email: string; readonly expiresAt: number; readonly admin: boolean }
  /** This host could not reach the cloud: the page offers to send the signed invite from the browser. */
  | { readonly view: "browser"; readonly answer: number; readonly email: string; readonly signed: string }
  | { readonly view: "refused"; readonly answer: number; readonly email: string };

async function made(options: CallOptions, form: FormData, email: string, answer: number): Promise<EmailState> {
  const target = scope(form);
  if (target.kind === "account") {
    return { view: "form", answer, email, error: message("error.refused") };
  }
  const result = await createInvite({ ...target, delivery: "cloud", email }, options);
  if (!result.ok) {
    const limit = result.error.kind === "refused" && result.error.code === "rate_limited";
    const said = refusal(result.error) ?? (await failed(result.error));
    return limit ? { view: "form", answer, email, limited: said } : { view: "form", answer, email, error: said };
  }
  const { invite, signed } = result.value;
  revalidatePath("/admin", "layout");
  const delivery = signed === null ? "refused" : await deliverInvite(signed);
  if (delivery === "delivered") {
    return { view: "sent", answer, email, expiresAt: invite.expiresAt, admin: invite.admin };
  }
  if ((delivery === "unreachable" || delivery === "no_cloud") && signed !== null) {
    return { view: "browser", answer, email, signed };
  }
  return { view: "refused", answer, email };
}

/** *Send invite*: the server makes and signs the invite, then this admin delivers it to Orchester Cloud. */
export async function emailInviteAction(previous: EmailState, form: FormData): Promise<EmailState> {
  const answer = previous.answer + 1;
  const typed = field(form, "email");
  const options = await actor(form);
  if (isMessage(options)) {
    return { view: "form", answer, email: typed, error: options };
  }
  const invalid = emailError(typed);
  const email = normalizeEmail(typed);
  if (invalid !== undefined || email === null) {
    return { view: "form", answer, email: typed, error: invalid ?? message("error.emailInvalid") };
  }
  return made(options, form, email, answer);
}

export type CodeState =
  | { readonly view: "form"; readonly answer: number; readonly error?: Message }
  /** The code, shown once: it lives only in this answer, never in a cookie, an address or a log. */
  | {
      readonly view: "code";
      readonly answer: number;
      readonly code: string;
      readonly expiresAt: number;
      /** Whole hours it works from now, as the server set it (*in 72 hours*). */
      readonly hours: number;
      readonly role: Role | null;
      readonly admin: boolean;
    };

/** *Make a code*, *Invite with a code*, *Account code*: the server makes a code and shows it this once. */
export async function codeInviteAction(previous: CodeState, form: FormData): Promise<CodeState> {
  const answer = previous.answer + 1;
  const options = await actor(form);
  if (isMessage(options)) {
    return { view: "form", answer, error: options };
  }
  const target = scope(form);
  const input: NewInvite = target.kind === "account" ? target : { ...target, delivery: "code" };
  const result = await createInvite(input, options);
  if (!result.ok) {
    return { view: "form", answer, error: refusal(result.error) ?? (await failed(result.error)) };
  }
  const { invite, code } = result.value;
  revalidatePath("/admin", "layout");
  if (code === null) {
    return { view: "form", answer, error: message("error.unavailable") };
  }
  const hours = Math.max(1, Math.round((invite.expiresAt - Date.now()) / 3_600_000));
  return { view: "code", answer, code, expiresAt: invite.expiresAt, hours, role: invite.role, admin: invite.admin };
}

export interface RevokeState {
  readonly revoked?: "told" | "later";
  readonly error?: Message;
}

/**
 * *Revoke*: the invite stops at once on the server. An email invite's revocation is then delivered to the cloud;
 * when the cloud cannot be told now, the page says so: the invite is dead on the server either way.
 */
export async function revokeInviteAction(_: RevokeState, form: FormData): Promise<RevokeState> {
  const options = await actor(form);
  if (isMessage(options)) {
    return { error: options };
  }
  const id = field(form, "invite");
  const result = await revokeInvite(id, options);
  if (!result.ok) {
    return { error: await failed(result.error) };
  }
  revalidatePath("/admin", "layout");
  const { revocation } = result.value;
  if (revocation === null) {
    return { revoked: "told" };
  }
  return { revoked: (await deliverRevocation(id, revocation)) === "delivered" ? "told" : "later" };
}
