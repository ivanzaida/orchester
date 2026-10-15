import { adminReturn } from "@/lib/authorize";
import { field } from "@/lib/form";
import { config } from "@/lib/server/config";
import { deliverInvite } from "@/lib/server/registry";
import { anonymous } from "@/lib/server/visitor";

/**
 * Where a team server admin's page sends an invite by email when its server cannot reach the cloud
 * (`docs/architecture/server.md`, "Invites"): the inviter's browser posts the invite the server signed, and the cloud
 * checks that signature itself, so the post needs no session and no token of this site. The answer is a page that
 * says how it went and leads back to the admin.
 */
export async function POST(request: Request): Promise<Response> {
  const form = await request.formData();
  const back = adminReturn(field(form, "return"));
  const invite = field(form, "invite");
  const result = invite === "" ? null : await deliverInvite(invite, await anonymous());
  const outcome = result === null ? "refused" : result.ok ? "sent" : result.error.kind === "refused" ? "refused" : "unavailable";
  const query = new URLSearchParams({ outcome, ...(back === null ? {} : { return: back }) });
  // 303: the browser follows with a GET, never posting the invite a second time.
  return Response.redirect(`${config().publicOrigin}/invites/delivered?${query.toString()}`, 303);
}
