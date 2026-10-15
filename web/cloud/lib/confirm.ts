import { message, type Message } from "@/lib/i18n";
import type { ApiFailure } from "@/lib/server/api";
import { failureMessage, waitMessage } from "@/lib/server/failures";

/**
 * What a failed *Confirm email* says (decision 65, `CVRpO`). A wrong password, another sign-up's link and a link that
 * is unknown, used or expired are one answer that never says which part was wrong. server.md row 65 names it
 * `401 confirm_failed`; the cloud as merged answers `422 invalid_link` (`docs/cloud-api.md`): both read alike. After 5
 * wrong passwords the cloud refuses the link for an hour (`429`). Both offer *Send a new link*.
 */
export function confirmFailure(failure: ApiFailure, now: number): { failed: boolean; error: Message } {
  if (failure.kind === "refused" && (failure.code === "confirm_failed" || failure.code === "invalid_link")) {
    return { failed: true, error: message("confirm.failed") };
  }
  if (failure.kind === "refused" && failure.code === "rate_limited") {
    return { failed: true, error: waitMessage("confirm.tooMany", failure.retryAfter ?? 3600, now) };
  }
  return { failed: false, error: failureMessage(failure, "error.tooManyTries", now) };
}
