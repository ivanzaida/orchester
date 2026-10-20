import { message, type Message } from "@/lib/i18n";
import type { ApiFailure } from "@/lib/server/api";

/**
 * What the email field says when the server refuses an invite by email that would repeat one (contract, "Server
 * API"): the address has an open invite, sent then (`invite_open`), or already has what the invite gives
 * (`already_member`: the project's membership, or an account on the server). `null` for any other failure.
 */
export function repeatedError(failure: ApiFailure, email: string, scope: "project" | "server"): Message | null {
  if (failure.kind !== "refused") {
    return null;
  }
  if (failure.code === "invite_open" && failure.sentAt !== undefined) {
    return { key: "invites.openInvite", params: { email }, when: failure.sentAt };
  }
  if (failure.code === "already_member") {
    return message(scope === "project" ? "invites.alreadyMember" : "invites.alreadyAccount", { email });
  }
  return null;
}
