"use server";

import { revalidatePath } from "next/cache";

import { field } from "@/lib/form";
import type { Message } from "@/lib/i18n";
import { actor, failed, isMessage } from "@/lib/server/act";
import { decideApproval } from "@/lib/server/team/auth";
import { revokeDevice } from "@/lib/server/team/people";

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
  revalidatePath("/admin", "layout");
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
  revalidatePath("/admin", "layout");
  return { revoked: true };
}
