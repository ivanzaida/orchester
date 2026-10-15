import { inspectLink, type LinkPurpose } from "@/lib/server/cloud";
import { CloudUnavailableError } from "@/lib/server/failures";
import { anonymous } from "@/lib/server/visitor";

/**
 * The address an emailed link is for, or `null` when the link is unknown, used or expired. Any other
 * failure is the cloud's and shows the error page.
 */
export async function linkEmail(token: string, purpose: LinkPurpose): Promise<string | null> {
  const result = await inspectLink(token, purpose, await anonymous());
  if (result.ok) {
    return result.value;
  }
  if (result.error.kind === "refused" && result.error.code === "invalid_link") {
    return null;
  }
  throw new CloudUnavailableError(result.error);
}
