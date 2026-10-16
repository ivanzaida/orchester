"use server";

import { field } from "@/lib/form";
import type { Message } from "@/lib/i18n";
import { actor, failed, isMessage } from "@/lib/server/act";
import { decideApproval } from "@/lib/server/team/auth";
import { revokeBrowser, revokeDevice } from "@/lib/server/team/people";

export interface ApprovalState {
  readonly decided?: "approved" | "refused";
  readonly error?: Message;
}

/** *Approve* at once, or *Refuse* once confirmed in place (`YY8pR`, `WV4AT`). A person answers these, never automation. */
export async function approvalAction(_: ApprovalState, form: FormData): Promise<ApprovalState> {
  const options = await actor(form);
  if (isMessage(options)) {
    return { error: options };
  }
  const approve = field(form, "op") === "approve";
  const result = await decideApproval(field(form, "approval"), approve, options);
  if (!result.ok) {
    return { error: await failed(result.error) };
  }
  // The row stays and says what was decided (`WV4AT`); the lists are read again on the next visit.
  return { decided: approve ? "approved" : "refused" };
}

export interface RevokeDeviceState {
  readonly revoked?: boolean;
  readonly error?: Message;
}

/** *Revoke* a joined computer, confirmed in place: it is signed out of the server at once. */
export async function revokeDeviceAction(_: RevokeDeviceState, form: FormData): Promise<RevokeDeviceState> {
  const options = await actor(form);
  if (isMessage(options)) {
    return { error: options };
  }
  const result = await revokeDevice(field(form, "device"), options);
  // `404`: the computer was already signed out, which is what was asked for.
  if (!result.ok && !(result.error.kind === "refused" && result.error.code === "not_found")) {
    return { error: await failed(result.error) };
  }
  // The row stays, replaced by what happened to it; the list is read again on the next visit.
  return { revoked: true };
}

/** *Revoke* a browser signed in to the admin, confirmed in place: it is signed out of the admin at once. */
export async function revokeBrowserAction(_: RevokeDeviceState, form: FormData): Promise<RevokeDeviceState> {
  const options = await actor(form);
  if (isMessage(options)) {
    return { error: options };
  }
  const result = await revokeBrowser(field(form, "browser"), options);
  // `404`: the browser was already signed out, or its 12 hours ended, which is what was asked for.
  if (!result.ok && !(result.error.kind === "refused" && result.error.code === "not_found")) {
    return { error: await failed(result.error) };
  }
  return { revoked: true };
}
