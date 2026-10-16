import { inspectLink, type LinkPurpose } from "@/lib/server/cloud";
import { CloudUnavailableError } from "@/lib/server/failures";
import { anonymous } from "@/lib/server/visitor";

/**
 * What the page knows about an emailed link before anything is pressed (`POST /auth/links/inspect`): `gone`
 * (unknown, used or expired), or `usable` with the address it is for. Any other failure is the cloud's and shows the
 * error page.
 */
export type LinkState = { readonly kind: "gone" } | { readonly kind: "usable"; readonly email: string };

export async function inspect(token: string, purpose: LinkPurpose): Promise<LinkState> {
  const result = await inspectLink(token, purpose, await anonymous());
  if (result.ok) {
    return { kind: "usable", email: result.value };
  }
  if (result.error.kind === "refused" && result.error.code === "invalid_link") {
    return { kind: "gone" };
  }
  throw new CloudUnavailableError(result.error);
}
