import { NextResponse } from "next/server";

import { field } from "@/lib/form";
import { authorizeFields, authorizeStart, backPath, withOutcome } from "@/lib/server/authorize";
import { config } from "@/lib/server/config";
import { sessionEnded } from "@/lib/server/failures";
import { isAdminPage, newState, type TripBack } from "@/lib/server/session";
import { serverInfo, signOut } from "@/lib/server/team/auth";
import { changeCloud } from "@/lib/server/team/settings";
import { anonymous, endSession, formIsGenuine, keepHandoff, ownOrigin, session, startTrip } from "@/lib/server/visitor";

/**
 * Starts a trip to Orchester Cloud's `/authorize` (contract, "The trip to the cloud's /authorize"): *Sign in with
 * Orchester Cloud* (`purpose=admin`), *Sign in again* (`purpose=again`: this sign-in ends and a new one starts, to use
 * two-factor authentication turned on since) or *Link to Orchester Cloud* (`purpose=link`). A plain form `POST` with
 * the CSRF token; the answer keeps a fresh `state` in a cookie and sends the browser to `/admin/trip/go`, whose form
 * posts the trip to the cloud: the link statement never goes into a URL.
 */
/**
 * A 303 to one of the admin's own pages, as a path: behind the team server the request's own URL has the private
 * listener's scheme (`http`), not the browser's, and a form post's redirect to another scheme is refused by the
 * cloud page's CSP (`form-action`).
 */
function to(_request: Request, path: string): NextResponse {
  return new NextResponse(null, { status: 303, headers: { location: path } });
}

/** `page`: the admin page *Sign in again* comes back to; `null` for a plain sign-in. */
async function signInTrip(request: Request, cloud: string, page: string | null): Promise<NextResponse> {
  const info = await serverInfo(await anonymous());
  if (!info.ok) {
    return to(request, withOutcome("/admin/signin", "failed", "unavailable"));
  }
  if (info.value.cloud === null || !info.value.cloud.accepting) {
    return to(request, withOutcome("/admin/signin", "failed", "cloud_off"));
  }
  const state = newState();
  await startTrip({ state, purpose: "admin", back: "signin", session: null, page });
  const fields = authorizeFields(await ownOrigin(), state, { purpose: "admin", server: info.value.cloud.server });
  await keepHandoff({ action: authorizeStart(cloud), fields });
  return to(request, "/admin/trip/go");
}

/**
 * *Sign in again*, in one step: this browser's session ends on the server and here, then the cloud's sign-in starts.
 * When the server cannot be told, the session stays and the page comes back as it was.
 */
async function againTrip(request: Request, cloud: string, page: string): Promise<NextResponse> {
  const current = await session();
  if (current !== null) {
    const ended = await signOut({ ...(await anonymous()), token: current.token });
    if (!ended.ok && !sessionEnded(ended.error)) {
      return to(request, page);
    }
    await endSession();
  }
  return signInTrip(request, cloud, page);
}

async function linkTrip(request: Request, cloud: string, back: TripBack): Promise<NextResponse> {
  const current = await session();
  if (current === null) {
    return to(request, "/admin/signin");
  }
  const result = await changeCloud({ link: "start" }, { ...(await anonymous()), token: current.token });
  if (!result.ok && sessionEnded(result.error)) {
    await endSession();
    return to(request, "/admin/signin");
  }
  if (!result.ok || result.value.statement === null) {
    return to(request, withOutcome(backPath("link", back), "link", "unavailable"));
  }
  const state = newState();
  await startTrip({ state, purpose: "link", back, session: current, page: null });
  const fields = authorizeFields(await ownOrigin(), state, { purpose: "link", statement: result.value.statement });
  await keepHandoff({ action: authorizeStart(cloud), fields });
  return to(request, "/admin/trip/go");
}

export async function POST(request: Request): Promise<NextResponse> {
  const form = await request.formData();
  const purpose = field(form, "purpose");
  const back: TripBack = field(form, "back") === "setup" ? "setup" : "settings";
  const again = purpose === "again" ? field(form, "page") : null;
  // *Sign in again* comes back only to one of the admin's own pages; any other is its first page.
  const againPage = again === null ? null : isAdminPage(again) ? again : "/admin";
  const page = purpose === "link" ? backPath("link", back) : (againPage ?? "/admin/signin");
  if (!(await formIsGenuine(form))) {
    // A page this sign-in is still on keeps it as it was; the sign-in page says the form expired.
    return to(request, againPage ?? withOutcome(page, purpose === "link" ? "link" : "failed", "expired"));
  }
  const cloud = config().cloudOrigin;
  if (cloud === null) {
    return to(request, page);
  }
  if (againPage !== null) {
    return againTrip(request, cloud, againPage);
  }
  return purpose === "link" ? linkTrip(request, cloud, back) : signInTrip(request, cloud, null);
}
