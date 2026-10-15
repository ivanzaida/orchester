import { inspectLink, type LinkPurpose } from "@/lib/server/cloud";
import { CloudUnavailableError } from "@/lib/server/failures";
import { anonymous } from "@/lib/server/visitor";

/**
 * What the page knows about an emailed link before anything is pressed: `gone` (unknown, used or expired), or
 * `usable` with the address it is for. The cloud does not have `POST /auth/links/inspect` yet and answers `404`;
 * then the address is `null`, the page shows the form without it, and the press finds out whether the link still
 * works. Any other failure is the cloud's and shows the error page.
 */
export type LinkState = { readonly kind: "gone" } | { readonly kind: "usable"; readonly email: string | null };

export async function inspect(token: string, purpose: LinkPurpose): Promise<LinkState> {
  const result = await inspectLink(token, purpose, await anonymous());
  if (result.ok) {
    return { kind: "usable", email: result.value };
  }
  if (result.error.kind === "refused" && result.error.code === "invalid_link") {
    return { kind: "gone" };
  }
  if (result.error.kind === "refused" && result.error.code === "not_found") {
    return { kind: "usable", email: null };
  }
  throw new CloudUnavailableError(result.error);
}
