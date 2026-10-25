import { adminReturn } from "@/lib/authorize";
import { field } from "@/lib/form";
import { seeOther } from "@/lib/server/redirect";
import { deliverInvite } from "@/lib/server/registry";
import { anonymous, keepDelivered } from "@/lib/server/visitor";

/**
 * Where a team server admin's page sends an invite by email when its server cannot reach the cloud
 * (`docs/architecture/server.md`, "Invites"): the inviter's browser posts the invite the server signed, and the cloud
 * checks that signature itself, so the post needs no session and no token of this site. The answer is a page that
 * says how it went and leads back to the admin of the server the cloud registered for that invite: never to an address
 * the post or a URL names (security review 1, minor 2). The outcome travels in a short-lived signed cookie.
 */
export async function POST(request: Request): Promise<Response> {
  const form = await request.formData();
  const invite = field(form, "invite");
  const result = invite === "" ? null : await deliverInvite(invite, await anonymous());
  if (result?.ok === true) {
    await keepDelivered({ outcome: "sent", back: adminReturn(`${result.value.address}/admin`) });
  } else {
    const outcome = result === null || result.error.kind === "refused" ? "refused" : "unavailable";
    await keepDelivered({ outcome, back: null });
  }
  // On the host the invite was posted to, which holds the cookie just set.
  return seeOther("/invites/delivered");
}
