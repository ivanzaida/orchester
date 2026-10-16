import { NextResponse } from "next/server";

import { field } from "@/lib/form";
import { authorizeFields, authorizeStart, backPath, withOutcome } from "@/lib/server/authorize";
import { config } from "@/lib/server/config";
import { sessionEnded } from "@/lib/server/failures";
import { newState, type TripBack } from "@/lib/server/session";
import { serverInfo } from "@/lib/server/team/auth";
import { changeCloud } from "@/lib/server/team/settings";
import { anonymous, endSession, formIsGenuine, keepHandoff, ownOrigin, session, startTrip } from "@/lib/server/visitor";

/**
 * Starts a trip to Orchester Cloud's `/authorize` (contract, "The trip to the cloud's /authorize"): *Sign in with
 * Orchester Cloud* (`purpose=admin`) or *Link to Orchester Cloud* (`purpose=link`). A plain form `POST` with the CSRF
 * token; the answer keeps a fresh `state` in a cookie and sends the browser to `/admin/trip/go`, whose form posts the
 * trip to the cloud: the link statement never goes into a URL.
 */
/**
 * A 303 to one of the admin's own pages, as a path: behind the team server the request's own URL has the private
 * listener's scheme (`http`), not the browser's, and a form post's redirect to another scheme is refused by the
 * cloud page's CSP (`form-action`).
 */
function to(_request: Request, path: string): NextResponse {
  return new NextResponse(null, { status: 303, headers: { location: path } });
}

async function signInTrip(request: Request, cloud: string): Promise<NextResponse> {
  const info = await serverInfo(await anonymous());
  if (!info.ok) {
    return to(request, withOutcome("/admin/signin", "failed", "unavailable"));
  }
  if (info.value.cloud === null || !info.value.cloud.accepting) {
    return to(request, withOutcome("/admin/signin", "failed", "cloud_off"));
  }
  const state = newState();
  await startTrip({ state, purpose: "admin", back: "signin", session: null });
  const fields = authorizeFields(await ownOrigin(), state, { purpose: "admin", server: info.value.cloud.server });
  await keepHandoff({ action: authorizeStart(cloud), fields });
  return to(request, "/admin/trip/go");
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
  await startTrip({ state, purpose: "link", back, session: current });
  const fields = authorizeFields(await ownOrigin(), state, { purpose: "link", statement: result.value.statement });
  await keepHandoff({ action: authorizeStart(cloud), fields });
  return to(request, "/admin/trip/go");
}

export async function POST(request: Request): Promise<NextResponse> {
  const form = await request.formData();
  const purpose = field(form, "purpose");
  const back: TripBack = field(form, "back") === "setup" ? "setup" : "settings";
  const page = purpose === "link" ? backPath("link", back) : "/admin/signin";
  if (!(await formIsGenuine(form))) {
    return to(request, withOutcome(page, purpose === "link" ? "link" : "failed", "expired"));
  }
  const cloud = config().cloudOrigin;
  if (cloud === null) {
    return to(request, page);
  }
  return purpose === "link" ? linkTrip(request, cloud, back) : signInTrip(request, cloud);
}
