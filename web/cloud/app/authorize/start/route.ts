import { readRequest, pageFor } from "@/lib/authorize";
import { field } from "@/lib/form";
import { config } from "@/lib/server/config";
import { keepAuthorizeRequest } from "@/lib/server/visitor";

/**
 * Where a team server's admin posts its request to `/authorize` (security review 1, B1): the purpose, the server's id
 * or its link statement, the admin's `state` and its callback, in the form's body. The request is kept sealed in a
 * cookie for this browser and the browser goes on to `/authorize`, whose address names only the callback: the link
 * statement never travels in a URL, a history or a log. The request is checked again on the page and on confirming,
 * and only one posted from the admin it answers (its `Origin` is the callback's) is kept.
 */
/**
 * Whether the post came from the admin it answers: its `Origin` is the callback's. A browser hides the origin of an
 * https page that posts to plain http (`Origin: null`, the Fetch standard's downgrade rule); that happens only with a
 * cloud on plain http, which this site allows on loopback alone (a local or test setup), so only there is a null
 * origin with an https callback taken.
 */
function fromItsAdmin(request: Request, returnTo: URL, base: string): boolean {
  const origin = request.headers.get("origin");
  if (origin === returnTo.origin) {
    return true;
  }
  return origin === "null" && returnTo.protocol === "https:" && base.startsWith("http:");
}

export async function POST(request: Request): Promise<Response> {
  const form = await request.formData();
  const names = ["purpose", "server", "statement", "state", "return"] as const;
  const asked = readRequest(Object.fromEntries(names.map((name) => [name, field(form, name) || undefined])));
  const base = config().publicOrigin;
  // Only the admin the answer goes back to may ask: a post from any other page (or none) leaves this browser's
  // request as it was (security review 2, 1).
  if (asked === null || !fromItsAdmin(request, asked.returnTo, base)) {
    return Response.redirect(`${base}/authorize`, 303);
  }
  await keepAuthorizeRequest(asked);
  return Response.redirect(`${base}${pageFor(asked)}`, 303);
}
