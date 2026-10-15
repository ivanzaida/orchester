import { redirect } from "next/navigation";

import { message, type Message, type MessageKey } from "@/lib/i18n";
import type { ApiFailure } from "@/lib/server/api";
import { endSession } from "@/lib/server/visitor";

/** What the person reads when the server refuses or does not answer; each page handles the codes it expects first. */

/** A wait of `retryAfter` seconds as *in N min, at HH:MM*; `key` names the sentence with `{{minutes}}` and `{{time}}`. */
export function waitMessage(key: MessageKey, retryAfter: number, now: number): Message {
  return message(key, { minutes: Math.max(1, Math.ceil(retryAfter / 60)) }, now + retryAfter * 1000);
}

/**
 * Any other failure: `rate_limited` reads as *too many tries*, a refused input as *did not accept this*, a refusal of
 * rights as *not allowed*, something gone as *no longer there*, and everything else (no answer, an internal error, an
 * answer this admin does not understand) as *not answering*.
 */
export function failureMessage(failure: ApiFailure, now: number): Message {
  if (failure.kind !== "refused") {
    return message("error.unavailable");
  }
  switch (failure.code) {
    case "rate_limited":
      return waitMessage("error.tooManyTries", failure.retryAfter ?? 60, now);
    case "invalid":
    case "bad_request":
      return message("error.refused");
    case "forbidden":
      return message("error.forbidden");
    case "not_found":
    case "no_such_user":
      return message("error.gone");
    default:
      return message("error.unavailable");
  }
}

/** A page that cannot be shown because the server failed; `app/error.tsx` shows it honestly. */
export class ServerUnavailableError extends Error {
  constructor(readonly failure: ApiFailure) {
    super(`the team server failed: ${failure.kind}${failure.kind === "refused" ? ` ${failure.code}` : ""}`);
    this.name = "ServerUnavailableError";
  }
}

export function sessionEnded(failure: ApiFailure): boolean {
  return failure.kind === "refused" && failure.code === "unauthorized";
}

/**
 * Forgets a session the server ended (`401 unauthorized`: signed out elsewhere, revoked, its 12 hours over, the
 * account disabled) and sends the person to sign in. For server actions and route handlers, which may change cookies.
 */
export async function leaveIfEnded(failure: ApiFailure): Promise<void> {
  if (sessionEnded(failure)) {
    await endSession();
    redirect("/admin/signin");
  }
}
