import { message, type Message, type MessageKey } from "@/lib/i18n";
import type { ApiFailure } from "@/lib/server/api";

/** What the visitor reads when the cloud refuses or does not answer; the pages handle the codes they expect first. */

/** A wait of `retryAfter` seconds as *in N min, at HH:MM*; `key` names the sentence with `{{minutes}}` and `{{time}}`. */
export function waitMessage(key: MessageKey, retryAfter: number, now: number): Message {
  return message(key, { minutes: Math.max(1, Math.ceil(retryAfter / 60)) }, now + retryAfter * 1000);
}

/**
 * Any other failure. `rate_limited` reads as `limitKey`; a refused input reads as *did not accept this*; everything
 * else (no answer, an internal error, an answer this site does not understand) as *not answering*.
 */
export function failureMessage(failure: ApiFailure, limitKey: MessageKey, now: number): Message {
  if (failure.kind !== "refused") {
    return message("error.unavailable");
  }
  if (failure.code === "rate_limited") {
    return waitMessage(limitKey, failure.retryAfter ?? 60, now);
  }
  return failure.code === "invalid" || failure.code === "bad_request"
    ? message("error.refused")
    : message("error.unavailable");
}

/** A page that cannot be shown because the cloud failed; `app/error.tsx` shows it honestly. */
export class CloudUnavailableError extends Error {
  constructor(readonly failure: ApiFailure) {
    super(`the cloud failed: ${failure.kind}${failure.kind === "refused" ? ` ${failure.code}` : ""}`);
    this.name = "CloudUnavailableError";
  }
}
